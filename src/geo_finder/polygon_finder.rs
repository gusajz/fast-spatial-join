// use geo::{polygon};

use cgmath::Point2;
use failure::Fail;
use geojson::Error as GeoJsonError;
use geojson::GeoJson;

use std::io;
use std::path;


use super::geo_finder_types::{PropertyMap, FindResult};
use geo::algorithm::bounding_rect::BoundingRect;
use geo::algorithm::centroid::Centroid;
use geo::algorithm::contains::Contains;
// use geo::algorithm::haversine_distance::HaversineDistance;
// use geo::algorithm::euclidean_distance::EuclideanDistance;
// use geo::algorithm::closest_point::ClosestPoint;
use geo_types;
use serde_json;
use spade;
use spade::rtree::RTree;
use std::collections::HashMap;
use std::convert::TryInto;
use std::io::prelude::*;
use std::fs::File;

use serde;

// TODO: import from base module (without super::super::)
// use super::super::cli_utils;

use log::{info};


// #[cfg(test)] #[macro_use]
// extern crate assert_matches;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
enum Area {
    Polygon(geo::Polygon<f64>),
    MultiPolygon(geo::MultiPolygon<f64>),
    Point(geo::Point<f64>),
}

// impl serde::Serialize for Area {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where S: serde::Serializer {
//         let mut state = serializer.serialize_struct("Color", 3)?;
//         state.serialize_field("r", &self.r)?;
//         state.serialize_field("g", &self.g)?;
//         state.serialize_field("b", &self.b)?;
//         state.end()
//     }
// }

impl Area where {
    /**
     * This is the actual Geometry, nothing is aproximate here.
     */
    // TODO: shouldn't return a reference?
    #[inline]
    fn mbr(&self) -> Option<geo_types::Rect<f64>> {
        match self {
            Area::Polygon(p) => p.bounding_rect(),
            Area::MultiPolygon(p) => p.bounding_rect(),
            Area::Point(p) => {
                let coord = geo::Coordinate::from((p.x(), p.y()));
                Some(geo_types::Rect {
                    min: coord,
                    max: coord,
                })
            }
        }
    }

    // TODO: shouldn't return a reference?
    #[inline]
    fn centroid(&self) -> Option<geo::Point<f64>> {
        match self {
            Area::Polygon(p) => p.centroid(),
            Area::MultiPolygon(p) => p.centroid(),
            Area::Point(p) => Some(geo::Point::from((p.x(), p.y()))),
        }
    }

    /**
     * No optimization
     */
    #[inline]
    fn contains_exact(&self, point: &geo::Point<f64>) -> bool {
        match self {
            Area::Polygon(p) => p.contains(point),
            Area::MultiPolygon(p) => p.contains(point),
            Area::Point(p) => p.x() == point.x() && p.y() == point.y(),
        }
    }

}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct IndexablePolygon {
    bbox: spade::BoundingRect::<Point2<f64>>,
    centroid: geo::Point<f64>,
    area: Area,
    properties: PropertyMap,
}

impl IndexablePolygon {

    pub fn new(feature: geojson::Feature) -> Result<IndexablePolygon, PolygonFinderError> {
        let geometry = feature
            .geometry
            .ok_or(PolygonFinderError::GeometryNotFound)?;
        let area: Area = match geometry.value {
            geojson::Value::Polygon(_) => {
                let geo_polygon: Result<geo_types::Polygon<f64>, PolygonFinderError> = geometry
                    .value
                    .try_into()
                    .map_err(PolygonFinderError::InvalidPolygon);
                geo_polygon.map(Area::Polygon)?
            }
            geojson::Value::MultiPolygon(_) => {
                let geo_multi_polygon: Result<geo_types::MultiPolygon<f64>, PolygonFinderError> =
                    geometry
                        .value
                        .try_into()
                        .map_err(PolygonFinderError::InvalidMultiPolygon);
                geo_multi_polygon.map(Area::MultiPolygon)?
            }
            geojson::Value::Point(_) => {
                let point_geometry: Result<geo_types::Point<f64>, PolygonFinderError> = geometry
                    .value
                    .try_into()
                    .map_err(PolygonFinderError::InvalidPoint);
                point_geometry.map(Area::Point)?
            }
            _ => return Err(PolygonFinderError::InvalidFeature),
        };

        let rect_bbox = area.mbr().ok_or(PolygonFinderError::GeometryNotFound)?;

        // TODO: check if this is ok for creating a bbox.
        let bbox = spade::BoundingRect::<Point2<f64>>::from_corners(
            &Point2::new(rect_bbox.min.x, rect_bbox.min.y),
            &Point2::new(rect_bbox.max.x, rect_bbox.max.y),
        );
    
        let properties_json: serde_json::map::Map<String, serde_json::value::Value> =
            feature.properties.unwrap_or_default();

        let mut properties = HashMap::new();
        for (k, v) in properties_json {
            match v {
                serde_json::Value::String(v_str) => {
                    properties.insert(k, v_str);
                }
                serde_json::Value::Number(v_num) => {
                    properties.insert(k, v_num.to_string());
                }
                other => {
                    return Err(PolygonFinderError::InvalidProperty(other))
                },
            }
        }

        Ok(IndexablePolygon {
            centroid: area.centroid().unwrap(), // TODO: unwrap is not cool
            bbox,
            area,
            properties,
        })
    }

    #[inline]
    pub fn bbox(&self) -> &spade::BoundingRect::<Point2<f64>> {
        return &self.bbox;
    }

}

impl spade::SpatialObject for IndexablePolygon {
    type Point = Point2<f64>;

    #[inline]
    fn mbr(&self) -> spade::BoundingRect<Self::Point> {
        return self.bbox;
    }

    /**
     * Distance optimized for and rtree (to the bbox, not exact).
     */
    #[inline]
    fn distance2(&self, point: &Self::Point) -> f64 {
        return self.bbox.distance2(point);

        // Alternatives.
        // let centroid = self.centroid;
        // let point = &geo::Point::from((point.x, point.y));
        // return centroid.haversine_distance(point);
        // return centroid.euclidean_distance(point);

        // scalar_result
        // return self.area.euclidean_distance2(&point);

        // self.area
        //     .distance2(&geo::Point::from((point.x, point.y)))
        //     .unwrap()
    }

    /**
     * Contains optimized for and rtree (to the bbox, not exact).
     */
    #[inline]
    fn contains(&self, point: &Self::Point) -> bool {
        // self.area.contains(&geo::Point::from((point.x, point.y)))
        // TODO: Should I use the bounding box?
        self.bbox.contains_point(point) 
    }
}

#[allow(dead_code)]
#[derive(Debug, Fail)]
pub enum PolygonFinderError {
    #[fail(display = "GeoJSON error: {}", _0)]
    Parse(GeoJsonError),
    #[fail(display = "Invalid property: {}", _0)]
    InvalidProperty(serde_json::Value),
    #[fail(display = "Invalid feature")]
    InvalidFeature,
    #[fail(display = "Feature collection not found")]
    FeatureCollectionNotFound,
    #[fail(display = "Geometry not found")]
    GeometryNotFound,
    #[fail(display = "Invalid polygon: {}", _0)]
    InvalidPolygon(GeoJsonError),
    #[fail(display = "Invalid multi polygon: {}", _0)]
    InvalidMultiPolygon(GeoJsonError),
    #[fail(display = "Invalid point polygon: {}", _0)]
    InvalidPoint(GeoJsonError),
    #[allow(dead_code)]
    #[fail(display = "Cannot calculate distance")]
    CannotCalculateDistance,
    #[fail(display = "I/O error: {}", _0)]
    Io(io::Error),
}


impl From<GeoJsonError> for PolygonFinderError {
    fn from(err: GeoJsonError) -> PolygonFinderError {
        info!("Error parsing geo-json: {}", err);
        PolygonFinderError::Parse(err)
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct PolygonFinder {
    // geo-json: GeoJson
    tree: RTree<IndexablePolygon>,
    neighbors_tests: usize
}

impl PolygonFinder {
    // pub fn new_from_file<P: AsRef<path::Path>>(
    //     file: P,
    // ) -> Result<PolygonFinder, PolygonFinderError> {
    //     fs::read_to_string(file)
    //         .map_err(PolygonFinderError::Io)
    //         .and_then(|json| PolygonFinder::new(&json))
    // }

    
    pub fn new<P: AsRef<path::Path>>(geojson_path: P) -> Result<PolygonFinder, PolygonFinderError> {
        let mut file = File::open(&geojson_path).unwrap();
        let mut file_contents = String::new();
        let _ = file.read_to_string(&mut file_contents);

        PolygonFinder::new_from_string(&file_contents)
    }

    pub fn new_from_string(geo_json_str: &str) -> Result<PolygonFinder, PolygonFinderError> {

        let geo_json = geo_json_str.parse::<GeoJson>()?;

        let feature_collection = if let GeoJson::FeatureCollection(ctn) = geo_json {
            ctn
        } else {
            return Err(PolygonFinderError::FeatureCollectionNotFound);
        };


        let polygons: Result<Vec<_>, _> = feature_collection
            .features
            .into_iter()
            .map(|f| {
                IndexablePolygon::new(f)
            })
            .collect();


        info!("Generating index");

        info!("Bulk load");
        let tree = RTree::bulk_load(polygons?);
        info!("Bulk load ended");

        Ok(PolygonFinder { tree, neighbors_tests: 10 })
    }


    pub fn find_by_point(&self, point: &geo::Point<f64>) -> Option<Box<FindResult>> {
        let tree_point = Point2::new(point.x(), point.y());
        // let result = self.tree.lookup(&tree_point);
        
        // Since a lot of bounding boxes may match. We need the nearest, and test them.
        let results = self.tree.nearest_n_neighbors(&tree_point, self.neighbors_tests);

        let point_geometry = Point2::new(point.x(), point.y());
        
        for result in results {
            /* 
             * But we want to optimize for the "not in any geometry" scenario, so we check everything with the 
             * bounding box before doing an exact lookup (that is much expensiver).
             */ 
            if  !result.bbox().contains_point(&point_geometry)  {
                return None
            }

            if  result.area.contains_exact(point) {

                // let distance = result.area.haversine_distance2(point);

                return Some(Box::new(FindResult { props: &result.properties, distance: 0.0 } ));
            }
        }

        return None;
    }

}

impl PolygonFinder {
    pub fn find(&self, latitude: f64, longitude: f64) -> Option<Box<FindResult>> {
        return self.find_by_point(&geo::Point::from((longitude, latitude)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // let hello: &str =
    const MEXICO_GEOJSON_STR: &str = include_str!("test_resources/mexico_states.json");
    #[allow(dead_code)]
    const INVALID_GEOJSON_STR_WITH_POINT: &str =
        include_str!("test_resources/mexico_states_with_point.json");
    const ONE_FEATURE_GEOJSON_STR: &str = include_str!("test_resources/one_feature_geojson.json");
    const MALFORMED_GEOJSON_STR: &str = include_str!("test_resources/malformed_geojson.json");

    const COLIMA_AGEBS_GEOJSON_STR: &str = include_str!("test_resources/agebs_colima.json");
    const COLIMA_ZIP_CODES_GEOJSON_STR: &str = include_str!("test_resources/zip_codes_colima.json");


    #[test]
    fn it_should_parse_a_valid_geojson() {
        let finder_result = PolygonFinder::new(&MEXICO_GEOJSON_STR);

        assert_eq!(finder_result.is_ok(), true);
    }

    // #[test]
    // fn it_should_fail_with_an_invalid_geojson() {
    //      TODO: we support points now. Change for other invalid geojson.
    //     let finder_result = PolygonFinder::new(&INVALID_GEOJSON_STR_WITH_POINT);
    //     assert_eq!(finder_result.err(), Some(PolygonFinderError::InvalidFeature));
    // }

    #[test]
    fn it_should_fail_with_an_invalid_geojson_without_feature_collection() {
        let finder_result = PolygonFinder::new(&ONE_FEATURE_GEOJSON_STR);
        match finder_result.err() {
            Some(PolygonFinderError::FeatureCollectionNotFound) => {}
            _ => {
                panic!("Wrong Error");
            }
        }
    }

    #[test]
    fn it_should_fail_with_an_malformed_geojson() {
        let finder_result = PolygonFinder::new(&MALFORMED_GEOJSON_STR);
        match finder_result.err() {
            Some(PolygonFinderError::Parse(geojson::Error::MalformedJson)) => {}
            _ => {
                panic!("Wrong Error");
            }
        }
    }

    #[test]
    fn it_should_find_a_point_in_a_polygon() {
        let finder = PolygonFinder::new(&MEXICO_GEOJSON_STR).unwrap();

        let result = finder.find_by_point(&geo::Point::from((-103.9936459, 23.1775256)));

        assert_eq!(result.is_some(), true);
    }

    #[test]
    fn it_should_not_find_a_point_outside_a_polygon() {
        let finder = PolygonFinder::new(&MEXICO_GEOJSON_STR).unwrap();

        let result = finder.find_by_point(&geo::Point::from((0.1, 0.1)));

        print!("RESULT: {:?}", result);
        assert_eq!(result.is_some(), false);
    }

    // Mexico tests.
    #[test]
    fn it_should_finds_easy_point_ageb() {
        let finder = PolygonFinder::new(&COLIMA_AGEBS_GEOJSON_STR).unwrap();

        let result = finder.find(19.320921, -103.8088817);

        assert_eq!(result.is_some(), true);
        assert_eq!(result.unwrap().props["CVEGEO"], "060030033");
    }


    #[test]
    fn it_should_find_coordinates_in_chihuahua() {
        let finder = PolygonFinder::new(&MEXICO_GEOJSON_STR).unwrap();

        let result = finder.find(28.14606, -105.34232);

        assert_eq!(result.is_some(), true);
        assert_eq!(result.unwrap().props["CVEGEO"], "08");
        
    }


    #[test]
    fn it_should_find_coordinates_in_veracruz_border() {
        let finder = PolygonFinder::new(&MEXICO_GEOJSON_STR).unwrap();

        let result = finder.find(22.22553, -97.90096);

        assert_eq!(result.is_some(), true);
        assert_eq!(result.unwrap().props["CVEGEO"], "30");
        
    }


    #[test]
    fn it_should_not_find_a_point_outside() {
        let finder = PolygonFinder::new(&COLIMA_AGEBS_GEOJSON_STR).unwrap();

        let result = finder.find_by_point(&geo::Point::from((0.0, 0.0)));

        assert_eq!(result.is_some(), false);
    }

    #[test]
    fn it_should_find_a_point_in_zip_codes() {
        let finder = PolygonFinder::new(&COLIMA_ZIP_CODES_GEOJSON_STR).unwrap();

        let result = finder.find(19.2740353, -103.7427995);

        assert_eq!(result.is_some(), true);

        let result = result.unwrap();
        assert_eq!(result.props["ZIP_CODE"], "28989");
        assert_eq!(result.props["STATE"], "col");
    }
}
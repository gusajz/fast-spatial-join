use indicatif::{ProgressBar, ProgressStyle};

fn create_progress_bar_template(
    quiet_mode: bool, 
    msg: &str, 
    length: Option<u64>, 
    template_progress: &str,
    template_spinner: &str,
) -> ProgressBar {
    let bar = match quiet_mode {
        true => ProgressBar::hidden(),
        false => {
            match length {
                Some(len) => ProgressBar::new(len),
                None => ProgressBar::new_spinner(),
            }
        }
    };

    bar.set_message(msg);
    match length.is_some() {
        true => bar
            .set_style(ProgressStyle::default_bar()
                .template(template_progress)
                .progress_chars("=> ")),
        false => bar
            .set_style(ProgressStyle::default_spinner()
                .template(template_spinner)),
    };

    bar.inc(0);    // Just to avoid the drawing after the log.

    bar
}
pub fn create_progress_bar_bytes(quiet_mode: bool, msg: &str, length: Option<u64>) -> ProgressBar {
    return create_progress_bar_template(
        quiet_mode, 
        msg, 
        length,
        "[{elapsed_precise}] {msg} {spinner:.green} [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} eta: {eta}",
        "[{elapsed_precise}] {msg} {spinner:.green}"
    );
}

pub fn create_progress_bar_count(quiet_mode: bool, msg: &str, length: Option<u64>) -> ProgressBar {
    return create_progress_bar_template(
        quiet_mode, 
        msg, 
        length,
        "[{elapsed_precise}] {msg} {spinner:.green} [{wide_bar:.cyan/blue}] {pos}/{len} eta: {eta}",
        "[{elapsed_precise}] {msg} {spinner:.green}"
    ); 
}
use epaper_album::config::CONFIG_PATH;
use epaper_album::selftest::{print_self_test_report, run_self_test};

fn main() {
    let report = run_self_test(CONFIG_PATH);
    print_self_test_report(&report);
}

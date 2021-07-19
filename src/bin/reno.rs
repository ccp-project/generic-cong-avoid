use generic_cong_avoid::{reno::Reno, Alg};
use tracing::{info, warn};

fn main() {
    tracing_subscriber::fmt::init();
    let (alg, ipc): (Alg<Reno>, _) = generic_cong_avoid::make_args("CCP Reno")
        .map_err(|err| warn!(?err, "bad argument"))
        .unwrap();

    info!(
        algorithm = "Reno",
        ?ipc,
        reports = ?alg.report_option,
        slow_start_mode = ?alg.ss,
        "starting CCP"
    );

    portus::start!(ipc.as_str(), alg).unwrap();
}

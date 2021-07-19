use generic_cong_avoid::{cubic::Cubic, Alg};
use tracing::{info, warn};

fn main() {
    tracing_subscriber::fmt::init();
    let (alg, ipc): (Alg<Cubic>, _) = generic_cong_avoid::make_args("CCP Cubic")
        .map_err(|err| warn!(?err, "bad argument"))
        .unwrap();

    info!(
        algorithm = "Cubic",
        ?ipc,
        reports = ?alg.report_option,
        slow_start_mode = ?alg.ss,
        "starting CCP"
    );

    portus::start!(ipc.as_str(), alg).unwrap();
}

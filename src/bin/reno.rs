extern crate clap;
extern crate time;

#[macro_use]
extern crate slog;

extern crate generic_cong_avoid;
extern crate portus;

use generic_cong_avoid::reno::Reno;

fn main() {
    let log = portus::algs::make_logger();
    let (cfg, ipc) = generic_cong_avoid::make_args("CCP Reno")
        .map_err(|e| warn!(log, "bad argument"; "err" => ?e))
        .unwrap_or_default();

    info!(log, "starting CCP"; 
        "algorithm" => "Reno",
        "ipc" => ipc.clone(),
        "reports" => ?cfg.report,
        "slow_start_mode" => ?cfg.ss,
    );

    generic_cong_avoid::start::<Reno>(ipc.as_str(), log, cfg);
}

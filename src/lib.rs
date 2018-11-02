//! Generic Cong Avoid is a higher-level API for [portus](https://crates.io/crates/portus)
//! which is suitable for traditional "congestion avoidance" algorithms.
//!
//! These algorithms comprise of an increase rule when new cumulative acknowledgements
//! arrive, and a decrease rule when congestion is detected. This crate provides implementations
//! of Reno and Cubic.
//!
//! The API retains the same structure as that of portus, but it is not necessary (nor possible)
//! to write custom datapath programs: the generic-cong-avoid layer collects a fixed set of measurements
//! detailed in [`GenericCongAvoidMeasurements`](./struct.GenericCongAvoidMeasurements.html).

extern crate clap;
extern crate time;
#[macro_use]
extern crate slog;
extern crate portus;

use std::collections::HashMap;
use portus::ipc::Ipc;
use portus::lang::Scope;
use portus::{CongAlg, Datapath, DatapathInfo, DatapathTrait, Report};

pub mod cubic;
pub mod reno;

mod bin_helper;
pub use bin_helper::{make_args, start};

pub const DEFAULT_SS_THRESH: u32 = 0x7fff_ffff;

/// The fixed list of measurements available to generic-cong-avoid algorithms
pub struct GenericCongAvoidMeasurements {
    pub acked: u32,
    pub was_timeout: bool,
    pub sacked: u32,
    pub loss: u32,
    pub rtt: u32,
    pub inflight: u32,
}

/// Configuration option: how often reports should be collected?
#[derive(Debug, Clone, Copy)]
pub enum GenericCongAvoidConfigReport {
    Ack,
    Rtt,
    Interval(time::Duration),
}

/// Configuration option: which implementation of slow start?
#[derive(Debug, Clone, Copy)]
pub enum GenericCongAvoidConfigSS {
    Datapath,
    Ccp,
}

/// An individual generic-cong-avoid flow
pub trait GenericCongAvoidFlow {
    fn curr_cwnd(&self) -> u32;
    fn set_cwnd(&mut self, cwnd: u32);
    fn increase(&mut self, m: &GenericCongAvoidMeasurements);
    fn reduction(&mut self, m: &GenericCongAvoidMeasurements);
    fn reset(&mut self) {}
}

/// An generic-cong-avoid algorithm, which contains a configuration
/// and can instantiate new flows.
pub trait GenericCongAvoidAlg {
    type Flow: GenericCongAvoidFlow;

    fn name() -> &'static str;
    fn args<'a, 'b>() -> Vec<clap::Arg<'a, 'b>> {
        vec![]
    }
    fn with_args(matches: clap::ArgMatches) -> Self;
    fn new_flow(&self, logger: Option<slog::Logger>, init_cwnd: u32, mss: u32) -> Self::Flow;
}

/// The generic-cong-avoid type which implements `portus::CongAlg`.
pub struct Alg<A: GenericCongAvoidAlg> {
    pub deficit_timeout: u32,
    pub init_cwnd: u32,
    pub report_option: GenericCongAvoidConfigReport,
    pub ss: GenericCongAvoidConfigSS,
    pub ss_thresh: u32,
    pub use_compensation: bool,
    pub logger: Option<slog::Logger>,
    pub alg: A,
}

impl<T: Ipc, A: GenericCongAvoidAlg> CongAlg<T> for Alg<A> {
    type Flow = Flow<T, A::Flow>;

    fn name() -> &'static str {
        A::name()
    }

    fn datapath_programs(&self) -> HashMap<&'static str, String> {
        let mut h = HashMap::default();
        h.insert(
            "DatapathIntervalProg",
            "
                (def
                (Report
                    (volatile acked 0)
                    (volatile sacked 0)
                    (volatile loss 0)
                    (volatile timeout false)
                    (volatile rtt 0)
                    (volatile inflight 0)
                )
                (reportTime 0)
                )
                (when true
                    (:= Report.inflight Flow.packets_in_flight)
                    (:= Report.rtt Flow.rtt_sample_us)
                    (:= Report.acked (+ Report.acked Ack.bytes_acked))
                    (:= Report.sacked (+ Report.sacked Ack.packets_misordered))
                    (:= Report.loss Ack.lost_pkts_sample)
                    (:= Report.timeout Flow.was_timeout)
                    (fallthrough)
                )
                (when (|| Report.timeout (> Report.loss 0))
                    (report)
                    (:= Micros 0)
                )
                (when (> Micros reportTime)
                    (report)
                    (:= Micros 0)
                )
            "
            .to_string(),
        );

        h.insert(
            "DatapathIntervalRTTProg",
            "
                (def (Report
                    (volatile acked 0)
                    (volatile sacked 0) 
                    (volatile loss 0)
                    (volatile timeout false)
                    (volatile rtt 0)
                    (volatile inflight 0)
                ))
                (when true
                    (:= Report.inflight Flow.packets_in_flight)
                    (:= Report.rtt Flow.rtt_sample_us)
                    (:= Report.acked (+ Report.acked Ack.bytes_acked))
                    (:= Report.sacked (+ Report.sacked Ack.packets_misordered))
                    (:= Report.loss Ack.lost_pkts_sample)
                    (:= Report.timeout Flow.was_timeout)
                    (fallthrough)
                )
                (when (|| Report.timeout (> Report.loss 0))
                    (report)
                    (:= Micros 0)
                )
                (when (> Micros Flow.rtt_sample_us)
                    (report)
                    (:= Micros 0)
                )
            "
            .to_string(),
        );

        h.insert(
            "AckUpdateProg",
            "
                (def (Report
                    (volatile acked 0)
                    (volatile sacked 0)
                    (volatile loss 0)
                    (volatile timeout false)
                    (volatile rtt 0)
                    (volatile inflight 0)
                ))
                (when true
                    (:= Report.acked (+ Report.acked Ack.bytes_acked))
                    (:= Report.sacked (+ Report.sacked Ack.packets_misordered))
                    (:= Report.loss Ack.lost_pkts_sample)
                    (:= Report.timeout Flow.was_timeout)
                    (:= Report.rtt Flow.rtt_sample_us)
                    (:= Report.inflight Flow.packets_in_flight)
                    (report)
                )
            "
            .to_string(),
        );

        h.insert(
            "SSUpdateProg",
            "
                (def (Report
                    (volatile acked 0)
                    (volatile sacked 0)
                    (volatile loss 0)
                    (volatile timeout false)
                    (volatile rtt 0)
                    (volatile inflight 0)
                ))
                (when true
                    (:= Report.acked (+ Report.acked Ack.bytes_acked))
                    (:= Report.sacked (+ Report.sacked Ack.packets_misordered))
                    (:= Report.loss Ack.lost_pkts_sample)
                    (:= Report.timeout Flow.was_timeout)
                    (:= Report.rtt Flow.rtt_sample_us)
                    (:= Report.inflight Flow.packets_in_flight)
                    (:= Cwnd (+ Cwnd Ack.bytes_acked))
                    (fallthrough)
                )
                (when (|| Report.timeout (> Report.loss 0))
                    (report)
                )

            "
            .to_string(),
        );

        h
    }

    fn new_flow(&self, control: Datapath<T>, info: DatapathInfo) -> Self::Flow {
        let init_cwnd = if self.init_cwnd != 0 {
            self.init_cwnd
        } else {
            info.init_cwnd
        };

        let mut s = Flow {
            control_channel: control,
            logger: self.logger.clone(),
            report_option: self.report_option,
            sc: Default::default(),
            ss_thresh: self.ss_thresh,
            rtt: 0,
            in_startup: false,
            mss: info.mss,
            use_compensation: self.use_compensation,
            deficit_timeout: self.deficit_timeout,
            init_cwnd,
            curr_cwnd_reduction: 0,
            last_cwnd_reduction: time::now().to_timespec() - time::Duration::milliseconds(500),
            alg: self.alg.new_flow(self.logger.clone(), init_cwnd, info.mss),
        };

        match (self.ss, self.report_option) {
            (GenericCongAvoidConfigSS::Datapath, _) => {
                s.sc = s.install_ss_update();
                s.in_startup = true;
            }
            (GenericCongAvoidConfigSS::Ccp, GenericCongAvoidConfigReport::Ack) => {
                s.sc = s.install_ack_update();
            }
            (GenericCongAvoidConfigSS::Ccp, GenericCongAvoidConfigReport::Rtt) => {
                s.sc = s.install_datapath_interval_rtt();
            }
            (GenericCongAvoidConfigSS::Ccp, GenericCongAvoidConfigReport::Interval(i)) => {
                s.sc = s.install_datapath_interval(i);
            }
        }

        s
    }
}

/// The generic-cong-avoid type which implements `portus::Flow`.
pub struct Flow<T: Ipc, A: GenericCongAvoidFlow> {
    alg: A,
    deficit_timeout: u32,
    init_cwnd: u32,
    report_option: GenericCongAvoidConfigReport,
    ss_thresh: u32,
    use_compensation: bool,
    control_channel: Datapath<T>,
    logger: Option<slog::Logger>,

    curr_cwnd_reduction: u32,
    last_cwnd_reduction: time::Timespec,

    in_startup: bool,
    mss: u32,
    rtt: u32,
    sc: Scope,
}

impl<I: Ipc, A: GenericCongAvoidFlow> portus::Flow for Flow<I, A> {
    fn on_report(&mut self, _sock_id: u32, m: Report) {
        let mut ms = self.get_fields(&m);

        if self.in_startup {
            // install new fold
            match self.report_option {
                GenericCongAvoidConfigReport::Ack => {
                    self.sc = self.install_ack_update();
                }
                GenericCongAvoidConfigReport::Rtt => {
                    self.sc = self.install_datapath_interval_rtt();
                }
                GenericCongAvoidConfigReport::Interval(i) => {
                    self.sc = self.install_datapath_interval(i);
                }
            }

            self.alg.set_cwnd(ms.inflight * self.mss);
            self.in_startup = false;
        }

        self.rtt = ms.rtt;
        if ms.was_timeout {
            self.handle_timeout();
            return;
        }

        ms.acked = self.slow_start_increase(ms.acked);

        // increase the cwnd corresponding to new in-order cumulative ACKs
        self.alg.increase(&ms);
        self.maybe_reduce_cwnd(&ms);
        if self.curr_cwnd_reduction > 0 {
            self.logger.as_ref().map(|log| {
                debug!(log, "in cwnd reduction"; "acked" => ms.acked / self.mss, "deficit" => self.curr_cwnd_reduction);
            });
            return;
        }

        self.update_cwnd();

        self.logger.as_ref().map(|log| {
            debug!(log, "got ack"; 
                "acked(pkts)" => ms.acked / self.mss,
                "curr_cwnd (pkts)" => self.alg.curr_cwnd() / self.mss,
                "inflight (pkts)" => ms.inflight,
                "loss" => ms.loss,
                "ssthresh" => self.ss_thresh,
                "rtt" => ms.rtt,
            );
        });
    }
}

impl<T: Ipc, A: GenericCongAvoidFlow> Flow<T, A> {
    /// Make no updates in the datapath, and send a report after an interval
    fn install_datapath_interval(&mut self, interval: time::Duration) -> Scope {
        self.control_channel
            .set_program(
                "DatapathIntervalProg",
                Some(&[("reportTime", interval.num_microseconds().unwrap() as u32)][..]),
            )
            .unwrap()
    }

    /// Make no updates in the datapath, and send a report after each RTT
    fn install_datapath_interval_rtt(&mut self) -> Scope {
        self.control_channel
            .set_program("DatapathIntervalRTTProg", None)
            .unwrap()
    }

    /// Make no updates in the datapath, but send a report on every ack.
    fn install_ack_update(&mut self) -> Scope {
        self.control_channel
            .set_program("AckUpdateProg", None)
            .unwrap()
    }

    /// Don't update acked, since those acks are already accounted for in slow start.
    /// Send a report once there is a drop or timeout.
    fn install_ss_update(&mut self) -> Scope {
        self.control_channel
            .set_program("SSUpdateProg", None)
            .unwrap()
    }

    fn update_cwnd(&self) {
        if let Err(e) = self
            .control_channel
            .update_field(&self.sc, &[("Cwnd", self.alg.curr_cwnd())])
        {
            self.logger.as_ref().map(|log| {
                warn!(log, "Cwnd update error";
                      "err" => ?e,
                );
            });
        }
    }

    fn get_fields(&mut self, m: &Report) -> GenericCongAvoidMeasurements {
        let sc = &self.sc;
        let ack = m
            .get_field(&String::from("Report.acked"), sc)
            .expect("expected acked field in returned measurement") as u32;

        let sack = m
            .get_field(&String::from("Report.sacked"), sc)
            .expect("expected sacked field in returned measurement") as u32;

        let was_timeout =
            m.get_field(&String::from("Report.timeout"), sc)
                .expect("expected timeout field in returned measurement") as u32;

        let inflight =
            m.get_field(&String::from("Report.inflight"), sc)
                .expect("expected inflight field in returned measurement") as u32;

        let loss = m
            .get_field(&String::from("Report.loss"), sc)
            .expect("expected loss field in returned measurement") as u32;

        let rtt = m
            .get_field(&String::from("Report.rtt"), sc)
            .expect("expected rtt field in returned measurement") as u32;

        GenericCongAvoidMeasurements {
            acked: ack,
            was_timeout: was_timeout == 1,
            sacked: sack,
            loss,
            rtt,
            inflight,
        }
    }

    fn handle_timeout(&mut self) {
        self.ss_thresh /= 2;
        if self.ss_thresh < self.init_cwnd {
            self.ss_thresh = self.init_cwnd;
        }

        self.alg.reset();
        self.alg.set_cwnd(self.init_cwnd);
        self.curr_cwnd_reduction = 0;

        self.logger.as_ref().map(|log| {
            warn!(log, "timeout"; 
                "curr_cwnd (pkts)" => self.init_cwnd / self.mss, 
                "ssthresh" => self.ss_thresh,
            );
        });

        self.update_cwnd();
        return;
    }

    fn maybe_reduce_cwnd(&mut self, m: &GenericCongAvoidMeasurements) {
        if m.loss > 0 || m.sacked > 0 {
            if self.deficit_timeout > 0
                && ((time::now().to_timespec() - self.last_cwnd_reduction)
                    > time::Duration::microseconds(
                        (f64::from(self.rtt) * self.deficit_timeout as f64) as i64,
                    )) {
                self.curr_cwnd_reduction = 0;
            }

            // if loss indicator is nonzero
            // AND the losses in the lossy cwnd have not yet been accounted for
            // OR there is a partial ACK AND cwnd was probing ss_thresh
            if m.loss > 0 && self.curr_cwnd_reduction == 0
                || (m.acked > 0 && self.alg.curr_cwnd() == self.ss_thresh)
            {
                self.alg.reduction(m);
                self.last_cwnd_reduction = time::now().to_timespec();
                self.ss_thresh = self.alg.curr_cwnd();
                self.update_cwnd();
            }

            self.curr_cwnd_reduction += m.sacked + m.loss;
        } else if m.acked < self.curr_cwnd_reduction {
            self.curr_cwnd_reduction -= (m.acked as f32 / self.mss as f32) as u32;
        } else {
            self.curr_cwnd_reduction = 0;
        }
    }

    fn slow_start_increase(&mut self, acked: u32) -> u32 {
        let mut new_bytes_acked = acked;
        if self.alg.curr_cwnd() < self.ss_thresh {
            // increase cwnd by 1 per packet, until ssthresh
            if self.alg.curr_cwnd() + new_bytes_acked > self.ss_thresh {
                new_bytes_acked -= self.ss_thresh - self.alg.curr_cwnd();
                self.alg.set_cwnd(self.ss_thresh);
            } else {
                let curr_cwnd = self.alg.curr_cwnd();
                if self.use_compensation {
                    // use a compensating increase function: deliberately overshoot
                    // the "correct" update to keep account for lost throughput due to
                    // infrequent updates. Usually this doesn't matter, but it can when
                    // the window is increasing exponentially (slow start).
                    let delta = f64::from(new_bytes_acked) / (2.0_f64).ln();
                    self.alg.set_cwnd(curr_cwnd + delta as u32);
                // let ccp_rtt = (rtt_us + 10_000) as f64;
                // let delta = ccp_rtt * ccp_rtt / (rtt_us as f64 * rtt_us as f64);
                // self.cwnd += (new_bytes_acked as f64 * delta) as u32;
                } else {
                    self.alg.set_cwnd(curr_cwnd + new_bytes_acked);
                }

                new_bytes_acked = 0
            }
        }

        new_bytes_acked
    }
}

#![feature(box_patterns, specialization, const_fn)]

extern crate simple_signal;
use simple_signal::Signal;

extern crate pyo3;
use pyo3::prelude::*;

extern crate pyportus;
use pyportus::raise;

extern crate generic_cong_avoid;
use generic_cong_avoid::{GenericCongAvoidFlow, GenericCongAvoidMeasurements, GenericCongAvoidConfigReport, GenericCongAvoidConfigSS, GenericCongAvoidAlg};

#[macro_use]
extern crate slog;

macro_rules! py_config_get {
    ($dict:expr, $key:expr) => (
        pyo3::FromPyObject::extract($dict.get_item($key).unwrap_or_else(|| {
            panic!("config missing required key '{}'", $key)
        })).unwrap_or_else(|e| {
            panic!("type mismatch for key '{}': {:?}", $key, e)
        })
    )
}

pub struct PyGenericCongAvoidAlg {
    pub py: Python<'static>,
    pub logger: slog::Logger,
    pub debug: bool,
    pub py_obj: PyObject,
}

#[py::class(gc,weakref,dict)]
pub struct Measurements {
    #[prop(get)]
    pub acked:       u32,
    #[prop(get)]
    pub was_timeout: bool,
    #[prop(get)]
    pub sacked:      u32,
    #[prop(get)]
    pub loss:        u32,
    #[prop(get)]
    pub rtt:         u32,
    #[prop(get)]
    pub inflight:    u32,
}

impl GenericCongAvoidAlg for PyGenericCongAvoidAlg {
    type Flow = Self;

    fn name() -> &'static str {
        "python+generic-cong-avoid"
    }

    fn with_args(_: clap::ArgMatches) -> Self {
        unreachable!("Python bindings construct their own arguments")
    }

    fn new_flow(&self, _logger: Option<slog::Logger>, init_cwnd: u32, mss: u32) -> Self::Flow {
        let args = PyTuple::new(self.py, &[init_cwnd, mss]);
        let flow_obj = self.py_obj.call_method1(self.py, "new_flow", args).unwrap_or_else(|e| {
            e.print(self.py);
            panic!("error calling new_flow()");
        });

        PyGenericCongAvoidAlg {
            py : self.py,
            logger: self.logger.clone(),
            debug: self.debug,
            py_obj: flow_obj,
        }
    }
}

impl GenericCongAvoidFlow for PyGenericCongAvoidAlg {
    fn curr_cwnd(&self) -> u32 {
        match self.py_obj.call_method0(self.py, "curr_cwnd") {
            Ok(ret) => {
                ret.extract(self.py).unwrap_or_else(|e| {
                    e.print(self.py); panic!(" curr_cwnd must return a u32")
                })
            }
            Err(e) => {
                e.print(self.py); panic!("call to curr_cwnd failed")
            }
        }
    }

    fn set_cwnd(&mut self, cwnd: u32) {
        if self.debug {
            debug!(self.logger, "set_cwnd"; "cwnd" => cwnd);
        }
        let args = PyTuple::new(self.py, &[cwnd]);
        match self.py_obj.call_method1(self.py, "set_cwnd", args) {
            Ok(_) => {}
            Err(e) => {
                e.print(self.py); panic!("call to set_cwnd failed")
            }
        };
    }

    fn reset(&mut self) {
        if self.debug {
            debug!(self.logger, "reset");
        }
        match self.py_obj.call_method0(self.py, "reset") {
            Ok(_) => {}
            Err(e) => {
                e.print(self.py); panic!("call to reset failed")
            }
        }
    }

    fn increase(&mut self, m: &GenericCongAvoidMeasurements) {
        if self.debug {
            debug!(self.logger, "increase()"; "acked" => m.acked, "was_timeout" => m.was_timeout, "sacked" => m.          sacked, "loss" => m.loss, "rtt" => m.rtt, "inflight" => m.inflight);
        }
        let m_wrapper = self.py.init(|_t| Measurements {
            acked : m.acked,
            was_timeout : m.was_timeout,
            sacked : m.sacked,
            loss : m.loss,
            rtt : m.rtt,
            inflight : m.inflight
        }).unwrap_or_else(|e| {
            e.print(self.py); panic!("increase(): failed to create Measurements object")
        });
        let args = PyTuple::new(self.py, &[m_wrapper]);
        match self.py_obj.call_method1(self.py, "increase", args) {
            Ok(_) => {}
            Err(e) => {
                e.print(self.py); panic!("call to increase failed")
            }
        };
    }

    fn reduction(&mut self, m: &GenericCongAvoidMeasurements) {
        if self.debug {
	    debug!(self.logger, "reduction()"; "acked" => m.acked, "was_timeout" => m.was_timeout, "sacked" => m.sacked, "loss" => m.loss, "rtt" => m.rtt, "inflight" => m.inflight);
        }
        let m_wrapper = self.py.init(|_t| Measurements {
            acked : m.acked,
            was_timeout : m.was_timeout,
            sacked : m.sacked,
            loss : m.loss,
            rtt : m.rtt,
            inflight : m.inflight
        }).unwrap_or_else(|e| {
            e.print(self.py); panic!("increase(): failed to create Measurements object")
        });
        let args = PyTuple::new(self.py, &[m_wrapper]);
        match self.py_obj.call_method1(self.py, "reduction", args) {
            Ok(_) => {}
            Err(e) => {
                e.print(self.py); panic!("call to reduction failed")
            }
        };
    }
}

#[py::modinit(py_generic_cong_avoid)]
fn init_mod(py: pyo3::Python<'static>, m: &PyModule) -> PyResult<()> {
    #[pyfn(m, "_start")]
    fn _py_start(
        py: pyo3::Python<'static>,
        ipc_str: String,
        alg: PyObject,
        debug: bool,
        config: &PyDict,
    ) -> PyResult<i32> {
        simple_signal::set_handler(&[Signal::Int, Signal::Term], move |_signals| {
            ::std::process::exit(1);
        });
        py_start(py, ipc_str, alg, debug, config)
    }

    Ok(())
}

fn py_start(
    py: pyo3::Python<'static>,
    ipc: String,
    alg: PyObject,
    debug: bool,
    config: &PyDict,
) -> PyResult<i32> {
    let log = portus::algs::make_logger();

    // Check args
    if let Err(e) = portus::algs::ipc_valid(ipc.clone()) {
        raise!(ValueError, e);
    };

    if config.len() < 1 {
        unreachable!("received empty config")
    }

    let py_cong_alg = PyGenericCongAvoidAlg {
        py,
        logger: log.clone(),
        py_obj: alg,
        debug,
    };

    let alg = generic_cong_avoid::Alg {
        deficit_timeout: py_config_get!(config, "deficit_timeout"),
        init_cwnd:       py_config_get!(config, "init_cwnd"),
        report_option:   match py_config_get!(config, "report") {
            "ack" => GenericCongAvoidConfigReport::Ack,
            "rtt" => GenericCongAvoidConfigReport::Rtt,
            val   => GenericCongAvoidConfigReport::Interval(
                        time::Duration::milliseconds(val.parse::<i64>().unwrap_or_else(|e| {
                            panic!("'report' key must either be 'ack', 'rtt', or an i64 representing the report interval in milliseconds. we detected that the key was not 'ack' or 'rtt', but failed to convert it to an i64: {:?}", e)
                        }))
                    )
        },
        ss        : match py_config_get!(config, "ss") {
            "datapath" => GenericCongAvoidConfigSS::Datapath,
            "ccp"      => GenericCongAvoidConfigSS::Ccp,
            _          => panic!("'ss' key must either be 'datapath' or 'ccp'")
        },
        ss_thresh : py_config_get!(config, "ss_thresh"),
        use_compensation : py_config_get!(config, "use_compensation"),
        logger: Some(log.clone()),
        alg: py_cong_alg,
    };

    generic_cong_avoid::start::<PyGenericCongAvoidAlg>(ipc.as_str(), log, alg);

    Ok(0)
}

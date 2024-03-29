use generic_cong_avoid::{
    GenericCongAvoidAlg, GenericCongAvoidConfigReport, GenericCongAvoidConfigSS,
    GenericCongAvoidFlow, GenericCongAvoidMeasurements,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::*;
use simple_signal::Signal;
use tracing::debug;

macro_rules! py_config_get {
    ($dict:expr, $key:expr) => {
        pyo3::FromPyObject::extract(
            $dict
                .get_item($key)
                .unwrap_or_else(|| panic!("config missing required key '{}'", $key)),
        )
        .unwrap_or_else(|e| panic!("type mismatch for key '{}': {:?}", $key, e))
    };
}

pub struct PyGenericCongAvoidAlg<'p> {
    pub py: Python<'p>,
    pub debug: bool,
    pub py_obj: PyObject,
}

#[pyclass(weakref, dict)]
pub struct Measurements {
    #[pyo3(get)]
    pub acked: u32,
    #[pyo3(get)]
    pub was_timeout: bool,
    #[pyo3(get)]
    pub sacked: u32,
    #[pyo3(get)]
    pub loss: u32,
    #[pyo3(get)]
    pub rtt: u32,
    #[pyo3(get)]
    pub inflight: u32,
}

impl GenericCongAvoidAlg for PyGenericCongAvoidAlg<'_> {
    type Flow = Self;

    fn name() -> &'static str {
        "python+generic-cong-avoid"
    }

    fn with_args(_: clap::ArgMatches) -> Self {
        unreachable!("Python bindings construct their own arguments")
    }

    fn new_flow(&self, init_cwnd: u32, mss: u32) -> Self::Flow {
        let args = PyTuple::new(self.py, &[init_cwnd, mss]);
        let flow_obj = self
            .py_obj
            .call_method1(self.py, "new_flow", args)
            .unwrap_or_else(|e| {
                e.print(self.py);
                panic!("error calling new_flow()");
            });

        PyGenericCongAvoidAlg {
            py: self.py,
            debug: self.debug,
            py_obj: flow_obj,
        }
    }
}

impl GenericCongAvoidFlow for PyGenericCongAvoidAlg<'_> {
    fn curr_cwnd(&self) -> u32 {
        match self.py_obj.call_method0(self.py, "curr_cwnd") {
            Ok(ret) => ret.extract(self.py).unwrap_or_else(|e| {
                e.print(self.py);
                panic!(" curr_cwnd must return a u32")
            }),
            Err(e) => {
                e.print(self.py);
                panic!("call to curr_cwnd failed")
            }
        }
    }

    fn set_cwnd(&mut self, cwnd: u32) {
        if self.debug {
            debug!(cwnd, "set_cwnd");
        }
        let args = PyTuple::new(self.py, &[cwnd]);
        match self.py_obj.call_method1(self.py, "set_cwnd", args) {
            Ok(_) => {}
            Err(e) => {
                e.print(self.py);
                panic!("call to set_cwnd failed")
            }
        };
    }

    fn reset(&mut self) {
        if self.debug {
            debug!("reset");
        }
        match self.py_obj.call_method0(self.py, "reset") {
            Ok(_) => {}
            Err(e) => {
                e.print(self.py);
                panic!("call to reset failed")
            }
        }
    }

    fn increase(&mut self, m: &GenericCongAvoidMeasurements) {
        if self.debug {
            debug!(
                acked = m.acked,
                was_timeout = m.was_timeout,
                sacked = m.sacked,
                loss = m.loss,
                rtt = m.rtt,
                inflight = m.inflight,
                "increase"
            );
        }
        let m_wrapper = Py::new(
            self.py,
            Measurements {
                acked: m.acked,
                was_timeout: m.was_timeout,
                sacked: m.sacked,
                loss: m.loss,
                rtt: m.rtt,
                inflight: m.inflight,
            },
        )
        .unwrap_or_else(|e| {
            e.print(self.py);
            panic!("increase(): failed to create Measurements object")
        });
        let args = PyTuple::new(self.py, &[m_wrapper]);
        match self.py_obj.call_method1(self.py, "increase", args) {
            Ok(_) => {}
            Err(e) => {
                e.print(self.py);
                panic!("call to increase failed")
            }
        };
    }

    fn reduction(&mut self, m: &GenericCongAvoidMeasurements) {
        if self.debug {
            debug!(
                acked = m.acked,
                was_timeout = m.was_timeout,
                sacked = m.sacked,
                loss = m.loss,
                rtt = m.rtt,
                inflight = m.inflight,
                "reduction"
            );
        }
        let m_wrapper = Py::new(
            self.py,
            Measurements {
                acked: m.acked,
                was_timeout: m.was_timeout,
                sacked: m.sacked,
                loss: m.loss,
                rtt: m.rtt,
                inflight: m.inflight,
            },
        )
        .unwrap_or_else(|e| {
            e.print(self.py);
            panic!("increase(): failed to create Measurements object")
        });
        let args = PyTuple::new(self.py, &[m_wrapper]);
        match self.py_obj.call_method1(self.py, "reduction", args) {
            Ok(_) => {}
            Err(e) => {
                e.print(self.py);
                panic!("call to reduction failed")
            }
        };
    }
}

#[allow(non_snake_case)]
#[pymodule]
#[pyo3(name = "py_generic_cong_avoid")]
fn init_mod(_py: Python, m: &PyModule) -> PyResult<()> {
    #[pyfn(m)]
    fn _start(
        py: pyo3::Python,
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

fn py_start<'p>(
    py: pyo3::Python<'p>,
    ipc: String,
    alg: PyObject,
    debug: bool,
    config: &PyDict,
) -> PyResult<i32> {
    // Check args
    if let Err(e) = portus::algs::ipc_valid(ipc.clone()) {
        return Err(PyErr::new::<PyValueError, _>(e));
    };

    if config.len() < 1 {
        unreachable!("received empty config")
    }

    tracing_subscriber::fmt::init();

    let py_cong_alg = PyGenericCongAvoidAlg {
        py,
        py_obj: alg,
        debug,
    };
    // SAFETY: Calling _start will block the Python program, so really we will hold the GIL for the
    // remainder of the program's lifetime, which is 'static.
    let py_cong_alg: PyGenericCongAvoidAlg<'static> = unsafe { std::mem::transmute(py_cong_alg) };
    let alg = generic_cong_avoid::Alg {
        deficit_timeout: py_config_get!(config, "deficit_timeout"),
        init_cwnd:       py_config_get!(config, "init_cwnd"),
        report_option:   match py_config_get!(config, "report") {
            "ack" => GenericCongAvoidConfigReport::Ack,
            "rtt" => GenericCongAvoidConfigReport::Rtt,
            val   => GenericCongAvoidConfigReport::Interval(
                        std::time::Duration::from_millis(val.parse::<u64>().unwrap_or_else(|e| {
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
        alg: py_cong_alg,
    };

    tracing::info!(?ipc, "starting Generic Cong Avoid algorithm");
    portus::start!(ipc.as_str(), alg).unwrap();
    Ok(0)
}

CCP Algorithm: Generic Congestion Avoidance
===========================================

This repository provides a higher-level API on top of CCP for the increase-decrease family of 
congestion control algorithms. It also implements TCP Reno and TCP Cubic using this API. 

To get started using this algorithm with CCP, please see our [guide](https://ccp-project.github.io/ccp-guide).


## Notes

- In order to use this algorithm for congestion control, you also need to install a CCP datapath.
If you see errors about not being able to install a datapath program, it means that you have
either not installed a datapath, or the IPC mechanism between the algorithm and datapath is not
configured properly.
- For a simple example of how to use this API, see `src/reno.rs`. 

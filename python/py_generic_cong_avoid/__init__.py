from .py_generic_cong_avoid import _start
import sys
import inspect

alg_method_signatures = {
    'new_flow' : ['self', 'init_cwnd', 'mss']
}
flow_method_signatures = {
    'curr_cwnd' : ['self'],
    'set_cwnd' : ['self', 'cwnd'],
    'increase' : ['self', 'm'],
    'reduction' : ['self', 'm'],
    'reset' : ['self']
}

def assert_implements_interface(obj_cls, C):
    obj_cls_name = obj_cls.__name__
    if not issubclass(obj_cls, C):
        raise Exception("{} must be a sublcass of {}".format(obj_cls_name, C.__name__))
    for m in C.__signature__.keys():
        if not m in obj_cls.__dict__:
            raise NotImplementedError("{} does not implement the required method {}".format(obj_cls_name, m))
        if inspect.getargspec(getattr(obj_cls, m)).args != C.__signature__[m]:
            raise NameError("{}.{} does not match the required parameters {}".format(
                obj_cls_name,
                m,
                '(' + ', '.join(C.__signature__[m]) + ')'
            ))
    return True

class AlgBase(object):
    __signature__ = alg_method_signatures
    pass
class FlowBase(object):
    __signature__ = flow_method_signatures
    pass

def start(ipc, alg, config={}, debug=False):
    assert_implements_interface(alg.__class__, AlgBase)
    if '__flow__' in alg.__class__.__dict__:
        flow_cls = alg.__class__.__dict__['__flow__']
        assert_implements_interface(flow_cls, FlowBase)
    return _start(ipc, alg, debug, {})

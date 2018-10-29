import sys
import generic_cong_avoid

class RenoFlow(generic_cong_avoid.FlowBase):
    def __init__(self, init_cwnd, mss):
        self.init_cwnd = init_cwnd
        self.cwnd = init_cwnd
        self.mss = mss

    def curr_cwnd(self):
        return self.cwnd

    def set_cwnd(self, cwnd):
        self.cwnd = cwnd

    def increase(self, m):
        self.cwnd += self.mss * (m.acked / self.cwnd)

    def reduction(self, m):
        self.cwnd /= 2.0
        self.cwnd = max(self.cwnd, self.init_cwnd)

    def reset(self):
        pass

class Reno(generic_cong_avoid.AlgBase):
    __flow__ = RenoFlow

    def new_flow(self, init_cwnd, mss):
        return Reno.__flow__(init_cwnd, mss)

reno = Reno()

generic_cong_avoid.start("netlink", reno, {}, debug=True)

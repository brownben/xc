import sys
import os

STDLIB_PATH = os.__file__.removesuffix("os.py")
"""
This is used to exclude the standard library from coverage. The `os` module is the
only module which has the `__file__` attribute.
"""


class Tracer:
    """Object that is used to store the tracing state"""

    def __init__(self):
        self.trace = set()
        self.dont_trace = set()
        self.lines = dict()

    @staticmethod
    def should_trace_file(filename):
        return not filename.startswith("<") and not filename.startswith(STDLIB_PATH)

    def line_callback(self, code_object, line_number):
        if id(code_object) in self.trace:
            lines = self.lines.get(code_object.co_filename)
            if lines:
                lines.add(line_number)
            return sys.monitoring.DISABLE

        if Tracer.should_trace_file(code_object.co_filename):
            self.trace.add(id(code_object))
            if code_object.co_filename not in self.lines:
                self.lines[code_object.co_filename] = set()

            self.lines[code_object.co_filename].add(line_number)

        return sys.monitoring.DISABLE

    def get_lines(self):
        return dict(self.lines)


tracer = Tracer()

sys.monitoring.use_tool_id(sys.monitoring.COVERAGE_ID, "xc")
sys.monitoring.set_events(sys.monitoring.COVERAGE_ID, sys.monitoring.events.LINE)
sys.monitoring.register_callback(
    sys.monitoring.COVERAGE_ID,
    sys.monitoring.events.LINE,
    tracer.line_callback,
)

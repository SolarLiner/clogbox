from sympy.codegen import ast


class Tuple(ast.Node):
    __slots__ = ('elements', *ast.Node.__slots__)
    _fields = ('elements', *ast.Node._fields)

    def __new__(cls, *args, **kwargs):
        if len(args) == 1 and len(kwargs) == 0:
            return args[0]
        return ast.Node.__new__(cls, *args, **kwargs)

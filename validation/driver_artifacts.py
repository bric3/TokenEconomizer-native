"""Canonical JSON values for comparing driver artifacts across runtimes."""


def numeric_values(value):
    if isinstance(value,float) and value.is_integer(): return int(value)
    if isinstance(value,dict): return {key:numeric_values(item) for key,item in value.items()}
    if isinstance(value,list): return [numeric_values(item) for item in value]
    return value

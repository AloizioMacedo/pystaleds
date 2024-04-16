# pystaleds

![Code Tests](https://github.com/AloizioMacedo/pystaleds/actions/workflows/tests.yml/badge.svg?branch=master)
[![Coverage Status](https://coveralls.io/repos/github/AloizioMacedo/pystaleds/badge.svg?branch=master)](https://coveralls.io/github/AloizioMacedo/pystaleds?branch=master)
![Linting](https://github.com/AloizioMacedo/pystaleds/actions/workflows/linting.yml/badge.svg?branch=master)

Tool to check for docstring stale status compared to function signature.

Compares thing such as order of arguments, type mismatches, absence of arguments etc.

## Installing

You can install the package directly via pip using

```bash
pip install pystaleds
```

You can also simply build the binary using this repository directly with Rust.
For instance,

```bash
cargo build -r
./target/release/pystaleds test_folder
```

would run the program to check the files inside `test_folder` in this repository.

## Example

Suppose we have a function `f` as below.

```python
def f(x):
    """This is my function.

    Args:
        x: This is my variable

    Returns:
        I just return whatever was passed to me.
    """
    return x
```

In a new change, we want to add a flag in order to reverse the argument or not.

```python
def f(x, reverse):
    """This is my function.

    Args:
        x: This is my variable

    Returns:
        I just return whatever was passed to me.
    """
    if reverse:
        return -x
    else:
        return x
```

Note that we didn't change the docstring to reflect that we have a new variable.
This is precisely the type of thing we want to identify.

Running v0.1.1 of `pystaleds`, we would get the following results for each one
of those files:

```bash
✅ Success!
```

```bash
ERROR pystaleds::rules_checking: test.py: Line 1: Args from function: [("x", None), ("reverse", None)]. Args from docstring: [("x", None)]
Error: found errors in 1 file
```

The `None` that is present in that log line pertains to the types of the arguments in
case they are type hinted.

Indeed, if our code were:

```python
def f(x: int, reverse: bool):
    """This is my function.

    Args:
        x (int): This is my variable

    Returns:
        int: I just return whatever was passed to me.
    """
    if reverse:
        return -x
    else:
        return x
```

we would get:

```bash
ERROR pystaleds::rules_checking: test.py: Line 1: Args from function: [("x", Some("int")), ("reverse", Some("bool"))]. Args from docstring: [("x", Some("int"))]
Error: found errors in 1 file
```

If we change our code to

```python
def f(x: int, reverse: bool):
    """This is my function.

    Args:
        x (int): This is my variable
        reverse: Whether to reverse before returning.

    Returns:
        int: I just return whatever was passed to me.
    """
    if reverse:
        return -x
    else:
        return x
```

we fix the issue! ✅ Success!

Note, however, that if you put mismatching types for the signature and the docstring,
it will again raise errors.

## Options

The only required argument is the path, which can be either a folder or an isolated
file. In case it is a folder, it will run through its contents recursively.

Optional boolean arguments include:

-   --allow-hidden (--ah): This will include hidden files (i.e., those starting with
    ".") in the directory traversal.
-   --break-on-empty-line (--be): This will consider an empty line as a signal that
    the arguments section of the docstring has ended.
-   --forbid-no-docstring (--nd): This will raise an error in case a docstring is
    absent in a function definition.
-   --forbid-no-args-in-docstring (--na): This will raise an error in case a docstring
    does not have an arguments section.
-   --forbid_untyped_docstrings (--nu): This will raise an error in case a docstring
    has untyped arguments.

Optional non-boolean arguments include:

-   --glob (-g): Allows passing a glob that will determine which files to consider.
    In order for this to work, the path given to the program must be a folder. Then,
    the glob will be considered having such folder as root.
-   --docstyle (-s): Allows selecting the specific docstyle as a source for parsing.
    Defaults to auto-detect, which will try both google and numpy and use the one
    that works. But can be chosen to be specifically google or numpy.

## Benchmarking

The benchmark below (done with [hyperfine](https://github.com/sharkdp/hyperfine))
includes the average times to run each checker tool on my machine across different
projects. The `test_folder` refers to the folder on the root of this repo with two
simple `.py` files.

If the time would be over 1 minute, it is indicated as NaN in the table, as I
just stopped the test.

| Checker                                                 | pandas [ms] | numpy [ms] | fastapi [ms] | test_folder [ms] |
| ------------------------------------------------------- | ----------- | ---------- | ------------ | ---------------- |
| pystaleds                                               | 21.7        | 17.9       | 19.2         | 2.6              |
| pystaleds (with tree-sitter parsing)                    | 616.0       | 370.4      | 102.5        | 4.1              |
| [pydoclint](https://github.com/jsh9/pydoclint)          | 7418        | 3513       | 714.6        | 62.5             |
| [darglint](https://github.com/terrencepreilly/darglint) | NaN         | NaN        | 1152         | 125.3            |
| [docsig](https://github.com/jshwi/docsig)               | NaN         | NaN        | 19353        | 527.0            |

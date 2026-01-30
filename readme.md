### The Project

This is a calculator intended to provide quick and accurate answers to simple everyday tasks.

### How to Run

To start the calculator, run one of the following commands:

**Release** (recommended for actual use)
```pwsh
cd ./gui; cargo run --release
```

**Debug**
```pwsh
cd ./gui; cargo run
```

These commands compile the project and run the executable
- `./gui/target/release/gui.exe` in release or
- `./gui/target/debug/gui.exe` in debug mode.

### How to Use

By default, the calculator window will be minimized and (in release mode) no icon will be visible in the taskbar.
To open the calculator window, press ALT+SPACE. To minimize the Window again, press ESC or click somewhere
outside the window. If you want to keep the window open, open the context menu by right-clicking somewhere on an
empty area of the window and selecting 'Pin the window'.

**Rounding**\
You can change the rounding of the displayed number by opening the corresponding menu using the gear icon at the
top right and inputting the desired value. The rounding is measured in significant digits.

**Defining Symbols**\
You can define two types of symbols: variables and functions.
- Variables: `var_123 = ...`
- Functions: `fun_123(param1, param2, param2) = ...`

These symbols will remain in available until 'Clear Symbols' is pressed.
Clearing the history does not clear the defined symbols.

### Limitations

- This project currently only works on Windows.
- Using large exponents somewhere in the formula _might_ cause the calculator to freeze up.
- When confronted with unusual text input, the calculator might suddenly decide to terminate.

### Internal Calculation Logic

Whenever possible, calculations are performed using rational numbers.
This enables exact results with no rounding errors.
(The final _displayed_ result will be rounded to the specified number of significant digits.)
In cases where it is not possible to use rationals, the calculator falls back
to using large floating point numbers. You can observe this whenever the result
includes something like '(1024-bit precision)'. The chosen size of the floating
point numbers means that in almost all cases, the result will still be accurate.
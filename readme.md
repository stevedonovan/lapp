Command-line parsing is essential for any program that needs to be run by
other people, and is a messy task with many corner cases: this is one wheel that should not be
reinvented for a project. It should not to be too ugly to use, either, like
the `getopt_long` POSIX interface for C programs.

This crate is a Rust implementation of the Lua library
[lapp](http://stevedonovan.github.io/Penlight/api/manual/08-additional.md.html#Command_line_Programs_with_Lapp).
Like [docopt](http://docopt.org/), it starts from the fact that you must output usage text anyway,
so why not extract flag names and types from that text? This is one of those ideas
that tends to happen multiple times - my first implementation was in 2009 and it is
now part of the Penlight Lua libraries;  docopt came somewhat later in about 2011.

Given that there is a [Rust implementation](https://github.com/docopt/docopt.rs) of
docopt, what is the justification for Lapp in Rust?  It is a good deal simpler to
use and understand, and fulfills the need for basic command-line interfaces that
don't involve subcommands, etc. The philosophy is "fail early and hard" - the
program quits if there are any errors and returns a non-zero code.

Consider a 'head' program that needs to be given a file and the number of lines
to echo to stdout:

```rust
// head.rs
extern crate lapp;

fn main() {
	let args = lapp::parse_args("
Prints out first n lines of a file
  -n, --lines (default 10) number of lines
  -v, --verbose
  <file> (string) input file name
	");

	let n = args.get_integer("lines");
	let verbose = args.get_bool("verbose");
	let file = args.get_string("file");
	// your magic goes here
}
```

There are _flags_ (both short and long forms) like `lines`, `verbose` and _positional arguments_
like `file`. Flags have an associated type - for `lines` this is deduced from the
default value, and for `file` it is specified explicitly.  Flags or arguments without
defaults must be specified - except for simple boolean flags, which default to false.

This does a fair amount of work for you, given that you had to write the usage
text anyway:

  - the usage 'mini-language' is fairly simple
  - command-line arguments are processed GNU-style. You may say `--lines 20`
    or `-n 20`; short flags can be combined `-vn20`. `--` indicates end of
    command-line processing
  - not providing positional arguments or required flags is an error
  - the `lines` flag value must be a valid integer and will be converted

So the idea is something that is straightforward for the programmer to use and
self-documenting enough for the user.

## Lapp mini-language

A significant line in a Lapp specification starts either with '-' (flags) or
'<' (positional arguments).  Flags may be '-s, --long','--long' or '-s'. Any other
lines are ignored.  Short flags may only be letters or numbers; long flags are
alphanumeric, plus '_' and '-'.

These significant lines may be followed by a type-default specifier in parens. It
is either a type, like '(string)' or a default value, like '(default 10)'. If not
present then the flag is a simple boolean flag, default false. The currently
supported types are:

  - string
  - integer (`i32`)
  - float (`f32`)
  - boolean
  - infile   (`Box<Read>`)  (can have "stdin" as default)
  - outfile  (`Box<Write>`) (can have "stdout" as default)
  - path (`PathBuf`)  (default will be tilde-expanded)

'(default <val>)' then the type is deduced from the value - either an integer or a
float if numerical, string otherwise. It is always possible to quote default
string values in single quotes, which you should do if the default value is not a
word. When in doubt, quote.

With version 0.3.0, it's also possible to specify both the type and a default,
e.g. "(integer default 0)" or "(path default ~/.bonzo)".

If there is no default value (except for simple flags) then that flag or argument
_must_ be specified on the command-line - they are _required_.

In addition, flags may be _multiple_ or _arrays_. Both are reprsented by a vector
of one of the base types, but are used differently. For example,

```
  -I, --include... (string) flag may appear multiple times
  -p, --ports (integer...) the flag value itself is an array
  <args> (string...)
  ...
  ./exe -I. --include lib
  ./exe --ports '9000 9100 9200'
  ./exe one two three
```

Array flags are lists separated _either_ with spaces _or_ with commas. (But if
you use commas, extra space will be trimmed.)

Multiple flags have '...' after the flag, array flags have '...' after the type.
The exception is positional flags, which are always multiple. This syntax does
not support default values, since the default value is well defined - an empty
vector.

_ranges_ are supported. "(1..10)" means an integer between
1 and 10 (inclusive!), and "(0.0..5.0)" means a floating point number
between 0.0 and 5.0.

Two convenient file types are provided, "infile" and "outfile". `get_infile()`
will return a `Box<Read>` and `get_outfile()` will return a `Box<Write>`. If the
argument is not a file that can be opened for either reading or writing, then
the program will quit. A default can be specified, so "(default stdin)" will
wrap up `io.stdin()` for you if the flag is not provided. (This is why we return
boxed trait objects rather than actual `File` objects - to handle this case.)

By default, the accessor functions exit the program on error. But for every method
like `args.get_string("flag")` there is an error-returning `args.get_string_result("flag")`.

## More Code Examples

Array-valued flags (multiple or array) are accessed with `args.get_strings("flag")`,
`args.get_integers("flag")`, etc.

If you'd like something other than the standard numeric types (`i32` or `f32`)
you can specify the type: `args.get::<u8>("flag")`.  It will then be an error to specify
integers outside 0..255. Simularly, `args.get_array::<u8>("flag")` will get an
integer-valued array flag as the desired type.

In fact, any type that implements the [FromStr](https://doc.rust-lang.org/1.15.1/std/str/trait.FromStr.html) trait will work.
In this example, we want to let the user enter integer values as hexadecimal.
It's necessary to specify any user types upfront, because otherwise **lapp** will complain about
unrecognized types.

```rust
extern crate lapp;
use std::str::FromStr;
use std::num::ParseIntError;

struct Hex {
    value: u64
}

impl FromStr for Hex {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self,Self::Err> {
        let value = u64::from_str_radix(s,16)?;
        Ok(Hex{value: value})
    }
}

let mut args = lapp::Args::new("
    --hex (hex default FF)
");
args.user_types(&["hex"]);
args.parse();

let res: Hex = args.get("hex");
println!("value was {}", res.value);
```


## Codegen

A criticism of this approach is that it isn't very strongly typed; it is
up to the programmer to use the correct `get_<type>` accessor for the flag, and
spelling mistakes are fatal at run-time. To get the boilerplate correct, there
is a tool in the 'src/bin' folder called `lapp-gen`.  In the examples folder
there is a `test.lapp` file:

```
Prints out first n lines of a file
  -n, --lines (default 10) number of lines
  -v, --verbose
  <file> (string) input file name

```

This is passed to `lapp-gen` as an environment variable (since we don't want to
confuse the command-line parameters here)

```
~/rust/lapp/examples$ LAPP_FILE='test.lapp vars' lapp-gen
    let lines = args.get_integer("lines");
    let verbose = args.get_bool("verbose");
    let file = args.get_string("file");
    let help = args.get_bool("help");
```

Lapp creates variable names out of flag names using a few simple rules; any '-'
is converted to '_'; if the flag name starts with a number or '_', then the name
is prepended with 'c_'.

You may test your spec by specifying just the file, and any command-line arguments:

```
~/rust/lapp/examples$ LAPP_FILE='test.lapp' lapp-gen
flag 'lines' value Int(10)
flag 'verbose' value Bool(false)
flag 'file' value Error("required flag file")
flag 'help' value Bool(false)
~/rust/lapp/examples$ LAPP_FILE='test.lapp' lapp-gen hello -v
flag 'lines' value Int(10)
flag 'verbose' value Bool(true)
flag 'file' value Str("hello")
flag 'help' value Bool(false)
~/rust/lapp/examples$ LAPP_FILE='test.lapp' lapp-gen hello -v --lines 30
flag 'lines' value Int(30)
flag 'verbose' value Bool(true)
flag 'file' value Str("hello")
flag 'help' value Bool(false)
~/rust/lapp/examples$ LAPP_FILE='test.lapp' lapp-gen hello -vn 40
flag 'lines' value Int(40)
flag 'verbose' value Bool(true)
flag 'file' value Str("hello")
flag 'help' value Bool(false)

```

The `mony.lapp` test file in `examples` gives all the permutations possible
with this version of Lapp.

The real labour saving codegen option is to generate a struct which is initialized
from lapp command-lines:

```rust
~/rust/lapp/examples$ LAPP_FILE='test.lapp struct:Args' lapp-gen
~/rust/lapp/examples$ cat test.lapp.inc
const USAGE: &'static str = "
Prints out first n lines of a file
  -n, --lines (default 10) number of lines
  -v, --verbose
  <file> (string) input file name

";
#[derive(Debug)]
struct Args {
	lines: i32,
	verbose: bool,
	file: String,
	help: bool,
}

impl Args {
	fn new() -> (Args,lapp::Args<'static>) {
		let args = lapp::parse_args(USAGE);
		(Args{
			lines: args.get_integer("lines"),
			verbose: args.get_bool("verbose"),
			file: args.get_string("file"),
			help: args.get_bool("help"),
		},args)
	}
}
```

And our program now looks like this, including the output `test.lapp.inc`.

```rust
// lines.rs
extern crate lapp;
include!("test.lapp.inc");

fn main() {
	let (values,args) = Args::new();
	if values.lines < 1 {
		args.quit("lines must be greater than zero");
	}
	println!("{:#?}",values);
}

```

(It would probably be more elegant to create a submodule, but then this would not
work in the examples folder except with subdirectories.)

## Limitations

In the last example it was necessary to explicitly _validate_ the arguments and quit
with an appropriate message. But most validation involves checking
more than one argument, and the more general solution is probably to have a `validate`
method stub in the generated code, where you can put your constraints.

Generally, however, I feel it's important to get a straightforward set of features right,
even if they are limited.  There are more general options for handling more complicated
command-line programs (for example, that support commands like 'cargo build' or 'git status')
and I intend to keep `lapp` as simple as possible, without extra dependencies.




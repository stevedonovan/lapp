extern crate lapp;
use std::env;
use std::fs::File;
use std::io;
use std::io::prelude::*;

const USAGE: &'static str = "
lapp-gen, generate Rust code from lapp specification files

ABOUT
lapp-gen verifies lapp specifications and then allows testing and code generation based
on those specifications.

USAGE
lapp-gen's behavior is specified using the environment variable `LAPP_GEN`; for instance:

    LAPP_GEN=my_spec.lapp lapp-gen <args>

LAPP_GEN can contain an additional field, seperated by a space. This is the code
generation mode, which defaults to 'validate'.

If lapp_gen is in 'validate' mode, any command-line arguments passed are parsed and the
results displayed. This allows you to prototype a command-line interface rapidly.

If the extra field is 'vars', it prints out a set of declarations that access the flags.
If 'struct', it prints out a suitable struct declaration for accessing the flags, which
is meant to be brought into your program using 'include!'.
";

enum Mode {
    Validate,
    Vars,
    Struct,
}

fn main() {
    // Get the user's instructions for what to do.
    let lapp_file_spec = env::var("LAPP_GEN").unwrap_or_else(|_| {
        print!("{}", USAGE);
        ::std::process::exit(1);
    });
    let parts: Vec<_> = lapp_file_spec.split_whitespace().collect();

    let mode = match if parts.len() > 1 {
        parts[1]
    } else {
        "validate"
    } {
        "validate" => Mode::Validate,
        "vars" => Mode::Vars,
        "struct" => Mode::Struct,
        _ => {
            panic!("mode must be blank or one of 'validate', 'vars', or 'struct'");
        }
    };

    // First part of the spec is the file to process.
    let lapp_file = parts[0];
    let mut f = File::open(lapp_file).expect(&format!("Unable to open {}. Error", lapp_file));
    let mut txt = String::new();
    f.read_to_string(&mut txt)
        .expect(&format!("Unable to read UTF-8 from {}. Error", lapp_file));

    let mut args = lapp::Args::new(&txt);

    match mode {
        Mode::Vars => {
            io::stdout()
                .write_all(&args.declarations("").into_bytes())
                .expect("Could not write to stdout. Error");
        }
        Mode::Struct => {
            io::stdout()
                .write_all(&args.declarations("Args").into_bytes())
                .expect("Could not write to stdout. Error");
        }
        Mode::Validate => {
            args.dump();
        }
    };
}

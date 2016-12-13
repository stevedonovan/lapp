// This little program has two purposes
// First, it verifies the _lapp specification file_, which must be specified
// in the environment variable LAPP_FILE. Any command-line arguments passed
// are parsed and the results displayed.
//
// If LAPP_FILE contains that filename and an addition field separated by space,
// then it generates code; if the extra field is 'vars', it prints out a set of
// declarations that access the flags; if 'vars:STRUCT_NAME', it writes out a
// suitable struct declaration for accessing the flags to a file 'SPEC-FILE.inc'.
// It is meant to be brought into your program using 'include!'.
//
// See test.lapp
extern crate lapp;
use std::io;
use std::env;
use std::fs::File;
use std::io::prelude::*;

fn main() {
    let lapp_file_spec = env::var("LAPP_FILE").expect("please set LAPP_FILE env var");
    let parts: Vec<_> = lapp_file_spec.split_whitespace().collect();
    let mode = if parts.len() > 1 { parts[1] } else {""};

    // first bit is the lapp command-line specification file
    let lapp_file = parts[0];
    let mut f = File::open(lapp_file).expect(&format!("cannot read {}",lapp_file));
    let mut txt = String::new();
    f.read_to_string(&mut txt).expect("bad test.lapp");

    let mut args = lapp::Args::new(&txt);

    if mode == "vars" || mode.starts_with("struct") {
        let struct_name = if mode != "vars" {
            match mode.find(':') {
                Some(idx) => {
                    let (_,s) = mode.split_at(idx+1);
                    s
                },
                None => args.quit("must be struct:STRUCT_NAME")
            }
        } else {
            ""
        };
        let dcls = args.declarations(struct_name);
        if mode != "vars" {
            let mut f = File::create(&format!("{}.inc",lapp_file)).expect("can't write");
            f.write_all(&dcls.into_bytes()).expect("can't write");
        } else {
            io::stdout().write_all(&dcls.into_bytes()).expect("can't write");
        }
    } else {
        args.dump();
    }

}

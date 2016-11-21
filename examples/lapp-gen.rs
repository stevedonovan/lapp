extern crate scanlex;
extern crate lapp;

use std::fs::File;
use std::io::prelude::*;

fn argn(idx: usize, def: &str) -> String {
    std::env::args().nth(idx).unwrap_or(def.to_string())
}

fn main() {
    let mode = argn(1,"dcl");
    let mut f = File::open("test.lapp").expect("cannot read test.lapp");
    let mut txt = String::new();
    f.read_to_string(&mut txt).expect("bad test.lapp");
    
    let mut args = lapp::Args::new(&txt);
    
    if mode == "dcl" {
        args.declarations();
    } else {
        args.parse();
        args.dump();
    }

/*
    args.parse();
    
    let output = args.get_string("output");
    let f = args.get_bool("f");
    let process = args.get_int("process");
    let k = args.get_int("k");
    let scale = args.get_float("scale");

    dbg!(output);
    dbg!(f);
    dbg!(process);
    dbg!(k);
    dbg!(scale);
 */   
}    

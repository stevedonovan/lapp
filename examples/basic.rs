extern crate lapp;
use std::io;
use std::io::prelude::*;
use std::error::Error;

fn run() -> Result<(),Box<Error>> {

    let args = lapp::parse_args("
    File input and output
      <in> (default stdin)
      <out> (default stdout)
    ");

    let inf = args.get_infile("in");
    let mut outf = args.get_outfile("out");

    let rdr = io::BufReader::new(inf);
    for line in rdr.lines() {
        let line = line?;
        write!(outf,"{}\n",line)?;
    }
    Ok(())

}

fn main() {
    run().expect("blew up");
}

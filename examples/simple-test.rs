extern crate lapp;

macro_rules! dbg (
    ($x:expr) => {
        println!("{}:{} {} = {:?}",file!(),line!(),stringify!($x),$x);
    }
);

const USAGE: &'static str = "
Testing Lapp
  -v, --verbose verbose flag
  -k   arb flag
  -o, --output (string)
  -p   (integer...)
  -h, --help help
  <out> (string...)
";

fn main() {
    let args = lapp::parse_args(USAGE);
        
    let verbose = args.get_bool("verbose");
    let k = args.get_bool("k");
    let output = args.get_string("output");
    let p = args.get_integers("p");
    let out = args.get_strings("out");
        
    dbg!(verbose);
    dbg!(k);
    dbg!(output);
    dbg!(p);
    dbg!(out);
    
}

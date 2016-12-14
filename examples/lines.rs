extern crate lapp;
include!("test.lapp.inc");

fn main() {
    let (values,args) = Args::new();
    if values.lines < 1 {
        args.quit("lines must be greater than zero");
    }
    println!("{:#?}",values);
}

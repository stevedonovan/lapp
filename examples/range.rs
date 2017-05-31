extern crate lapp;

fn main() {

    let args = lapp::parse_args("
    Integer range
      <in> (1..10) a number!
    ");

    let inf = args.get_integer("in");
    println!("{}",inf);
}


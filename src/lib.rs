//! `lapp` provides a straightforward way to parse command-line
//! arguments, using the _usage text_ as a pattern.
//!
//! # Example
//! ```
//! extern crate lapp;
//!
//! let args = lapp::parse_args("
//!    A test program
//!    -v,--verbose  verbose output
//!    -k         (default 10)
//!    -s, --save (default 'out.txt')
//!    <out>      (default 'stdout')
//! ");
//! assert_eq!(args.get_bool("verbose"),false);
//! assert_eq!(args.get_integer("k"),10);
//! assert_eq!(args.get_string("save"),"out.txt");
//! assert_eq!(args.get_string("out"),"stdout");
//! ```
//!
//! The usage text or _specification_ follows these simple rules:
//! line begining with one of '-short, --long', '--long' or '-short' (flags)
//! or begining with <name> (positional arguments).
//! These may be followed by a type/default specifier (<type>) - otherwise considered a bool flag
//! with default `false`. This specifier can be a type (like '(integer)') or a default,
//! like '(default 10)`. If there's a default, the type is infered from the value - can always
//! use single quotes to insist that the flag value is a string. Otherwise this flag is
//! _required_ and must be present!
//!
//! The currently supported types are 'string','integer','bool' and 'float'. There are
//! corresponding access methods like `get_string("flag")` and so forth.
//!
//! The flag may be followed by '...' (e.g '-I... (<type>)') and it is then a _multiple_
//! flag; its value will be a vector. This vector may be empty (flag is not required).
//! If the '...' appears inside the type specifier (e.g. '-p (integer...)') then
//! the flag is expecting several space-separated values (like -p '10 20 30'); it is also
//! represented by a vector.
//!
//! Rest of line (or any other kind of line) is ignored.
//!
//! lapp scans command-line arguments using GNU-style short and long flags.
//! Short flags may be combined, and may immediately followed by a value, e.g '-vk5'.
//! As an extension, you can say '--flag=value' or '-f:value'.

use std::process;
use std::env;
use std::io;
use std::io::{Write,Read};
use std::error::Error;

mod strutil;
mod types;
mod flag;
use types::*;
use flag::Flag;

pub struct Args<'a> {
    flags: Vec<Flag>,
    pos: usize,
    text: &'a str,
    varargs: bool,
}

impl <'a> Args<'a> {
    /// provide a _usage string_ from which we extract flag definitions
    pub fn new(text: &'a str) -> Args {
        Args{flags: Vec::new(), pos: 0, text: text, varargs: false}
    }

    /// bail out of program with non-zero return code.
    /// May force this to panic instead with the
    /// EASY_DONT_QUIT_PANIC environment variable.
    pub fn quit(&self, msg: &str) -> ! {
        let path = env::current_exe().unwrap();
        let exe = path.file_name().unwrap();
        let text = format!("{} error: {}",exe.to_string_lossy(),msg);
        if env::var("RUST_BACKTRACE").is_ok() {
            panic!(text);
        } else {
            writeln!(&mut io::stderr(),"{}",text).unwrap();
            process::exit(1);
        }
    }

    /// create suggested variable or struct declarations for accessing the flags...
    pub fn declarations(&mut self, struct_name: &str) -> String {
        if let Err(e) = self.parse_spec() {
            self.quit(e.description());
        }
        let mut res = String::new();
        if struct_name.len() > 0 {
            res += &format!("const USAGE: &'static str = \"\n{}\";\n",self.text);
            res += &format!("#[derive(Debug)]\nstruct {} {{\n",struct_name);
            for f in &self.flags {
                res += &format!("\t{}: {},\n",f.rust_name(),f.rust_type());
            }
            res += &format!(
                "}}\n\nimpl {} {{\n\tfn new() -> ({},lapp::Args<'static>) {{\n",
                struct_name,struct_name);
            res += &format!(
                "\t\tlet args = lapp::parse_args(USAGE);\n\t\t({}{{\n",struct_name);
            for f in &self.flags {
                res += &format!("\t\t\t{}: {},\n",f.rust_name(),f.getter_name());
            }
            res += &format!("\t\t}},args)\n\t}}\n}}\n\n");
        } else {
            for f in &self.flags {
                res += &format!("    let {} = {};\n",
                    f.rust_name(),f.getter_name());
            }
        }
        res
    }

    pub fn dump(&mut self) {
        self.parse();
        for f in &self.flags {
            println!("flag '{}' value {:?}",f.long,f.value);
        }
    }

    fn parse(&mut self) {
        if let Err(e) = self.parse_spec() { self.quit(e.description()); }
        let v: Vec<String> = env::args().skip(1).collect();
        if let Err(e) = self.parse_command_line(v) { self.quit(e.description()); }
    }

    fn parse_spec(&mut self) -> Result<()> {
        for line in self.text.lines() {
            self.parse_spec_line(line)?;
        }
        if let Err(_) = self.flags_by_long("help") {
            self.parse_spec_line("   -h,--help this help").unwrap();
        }
        Ok(())
    }


    fn parse_spec_line(&mut self, mut slice: &str) -> Result<()> {
        use strutil::*;

        if let Some(idx) = slice.find(|c: char| ! c.is_whitespace()) {
            let mut flag: Flag = Default::default();
            let mut is_positional = false;
            slice = &slice[idx..];
            let is_flag = starts_with(&mut slice,"-");
            let mut long_flag = starts_with(&mut slice,"-");
            if is_flag && ! long_flag { // short flag
                flag.short = (&slice[0..1]).chars().next().unwrap();
                if ! flag.short.is_alphanumeric() {
                    return error(format!("{:?} isn't allowed: only letters or digits in short flags",flag.short));
                }
                slice = &slice[1..];
                if let Some(0) = slice.find(|c: char| c.is_alphanumeric()) {
                   return error(format!("short flags should have one character"));
                }
                if starts_with(&mut slice,",") {
                    slice = skipws(slice);
                    long_flag = starts_with(&mut slice,"--");
                    if ! long_flag {
                        return error("expecting long flag after short flag");
                    }
                }
            }
            if long_flag {
                let idx = slice.find(|c: char| ! (c.is_alphanumeric() || c == '_' || c == '-'))
                    .unwrap_or(slice.len());
                let parts = slice.split_at(idx);
                flag.long = parts.0.to_string();
                slice = parts.1;
                if slice.len() > 0 && ! (slice.starts_with(" ") || slice.starts_with("."))  {
                    return error(format!("long flags can only contain letters, numbers, '_' or '-'"));
                }
            } else
            if starts_with(&mut slice, "<") { // positional argument
                flag.long = grab_upto(&mut slice, ">")?;
                self.pos = self.pos + 1;
                flag.pos = self.pos;
                is_positional = true;
            }
            if flag.long == "" && flag.short == '\0' {
                // not a significant line, ignore!
                return Ok(());
            }
            if flag.long == "" { // just a short flag
                flag.long = flag.short.to_string();
            }
            slice = skipws(slice);
            if starts_with(&mut slice,"...") {
                flag.is_multiple = true;
                slice = skipws(slice);
            }
            if starts_with(&mut slice,"(") {
                let r = grab_upto(&mut slice, ")")?;
                let mut rest = r.as_str();
                let multable = ends_with(&mut rest,"...");
                if let Some((b1,b2)) = split_with(rest,"..") {
                    // bounds on a number type
                    flag.set_range_constraint(b1,b2)?;
                } else {
                    // default VALUE or TYPE
                    let parts: Vec<_> = rest.split_whitespace().collect();
                    if parts.len() == 0 {
                        return error(format!("nothing inside type specifier"));
                    }
                    if parts.len() == 2 {
                        if parts[0] == "default" {
                            flag.defval = Value::from_value(parts[1])?;
                            flag.vtype = flag.defval.type_of();
                        } else {
                            return error(format!("expecting (default <value>)"));
                        }
                    } else {
                        flag.vtype = Type::from_name(parts[0])?;
                    }
                }
                // if type is followed by '...' then the flag is also represented
                // by a vector (e.g. --ports '8080 8081 8082').
                // UNLESS it is a positional argument,
                // where it is considered multiple!
                if multable {
                    flag.defval = Value::empty_array();
                    if is_positional {
                        flag.is_multiple = true;
                        if self.varargs {
                            return error("only last argument can occur multiple times");
                        }
                        self.varargs = true;
                    } else { // i.e the flag type is an array of a basic scalar type
                        flag.vtype = flag.vtype.create_empty_array();
                    }
                }
                if flag.is_multiple {
                    flag.value = Value::empty_array();
                }
            } else {
                flag.defval = Value::Bool(false);
                flag.vtype = Type::Bool;
            }
            if slice.len() > 0 {
                flag.help = skipws(slice).to_string();
            }

            // it is an error to specify a flag twice...
            if self.flags_by_long_ref(&flag.long).is_ok() {
                return error(format!("flag {:?} already defined",flag.long));
            }
            self.flags.push(flag);
        }
        Ok(())

    }

    fn flags_by_long(&mut self, s: &str) -> Result<&mut Flag> {
        self.flags.iter_mut()
            .filter(|&ref f| f.long == s)
            .next().ok_or(LappError(format!("no long flag '{}'",s)))
    }

    fn flags_by_long_ref(&self, s: &str) -> Result<&Flag> {
        self.flags.iter()
            .filter(|&f| f.long == s)
            .next().ok_or(LappError(format!("no long flag '{}'",s)))
    }

    fn flags_by_short(&mut self, ch: char) -> Result<&mut Flag> {
        self.flags.iter_mut()
            .filter(|&ref f| f.short == ch)
            .next().ok_or(LappError(format!("no short flag '{}'",ch)))
    }

    fn flags_by_pos(&mut self, pos: usize) -> Result<&mut Flag> {
        self.flags.iter_mut()
            .filter(|&ref f| f.pos == pos)
            .next().ok_or(LappError(format!("no positional arg '{}'",pos)))
    }

    fn parse_command_line(&mut self, v: Vec<String>) -> Result<()> {
        use strutil::*;
        let mut iter = v.into_iter();

        fn nextarg(name: &str, ms: Option<String>) -> Result<String> {
            if  ms.is_none() {return error(format!("no value for flag '{}'",name));}
            Ok(ms.unwrap())
        };

        // flags _may_ have the value after a = or : delimiter
        fn extract_flag_value(s: &mut &str) -> String {
            if let Some(idx) = s.find(|c: char| c == '=' || c == ':') {
               let rest = (&s[idx+1..]).to_string();
               *s = &s[0..idx];
               rest
            } else {
               "".to_string()
           }
        }

        let mut parsing = true;
        let mut k = 1;
        while let Some(arg) = iter.next() {
            let mut s = arg.as_str();
             if parsing && starts_with(&mut s, "--") { // long flag
                if s.len() == 0 { // plain '--' means 'stop arg processing'
                    parsing = false;
                } else {
                    let mut rest = extract_flag_value(&mut s);
                    let mut flag = self.flags_by_long(s)?;
                    if flag.vtype != Type::Bool { // then it needs a value....
                        if rest == "" {  // try grab the next arg
                            rest = nextarg(s,iter.next())?;
                        }
                        flag.set_value_from_string(&rest)?;
                    } else {
                        flag.set_value(Value::Bool(true))?;
                    }
                }
            } else
            if parsing && starts_with(&mut s,"-") { // short flag
                // there can be multiple short flags
                // although only the last one can take a value
                let mut chars = s.chars();
                while let Some(ch) = chars.next() {
                    let mut flag = self.flags_by_short(ch)?;
                    if flag.vtype != Type::Bool {
                        let mut rest: String = chars.collect();
                        if rest == "" {
                            rest = nextarg(&flag.long,iter.next())?;
                        }
                        flag.set_value_from_string(&rest)?;
                        break;
                    } else {
                       flag.set_value(Value::Bool(true))?;
                    }
                }
            } else {  // positional argument
                let mut flag = self.flags_by_pos(k)?;
                flag.set_value_from_string(s)?;
                // multiple arguments are added to the vector value
                if ! flag.is_multiple {
                    k += 1;
                }

            }
        }


        // display usage if help is requested
        if let Ok(ref flag) = self.flags_by_long_ref("help") {
            if flag.is_set {
                let mut text = self.text;
                if text.starts_with("\n") {
                    text = &text[1..];
                }
                println!("{}",text);
                process::exit(0);
            }
        }

        // fill in defaults. If a default isn't available it's
        // a required flag. If not specified the flag value is set to an error
        for flag in &mut self.flags {
            flag.check()?;
        }
        Ok(())
    }

    fn maybe_flag_value (&self, name: &str) -> Option<&Value> {
        if let Ok(ref flag) = self.flags_by_long_ref(name) {
           if flag.value.is_none() {
                None
            } else {
                Some(&flag.value)
            }
        } else {
            self.bad_flag(name,"unknown")
        }
    }

    fn maybe_flag<T, F: Fn(&Value) -> Result<T>> (&self, name: &str, extract: F) -> Option<T> {
        if let Some(value) = self.maybe_flag_value(name) {
            match extract(value) {
                Ok(v) => Some(v),
                Err(e) => self.bad_flag(name,e.description())
            }
        } else {
            None
        }
    }

    /// get flag as a string
    pub fn maybe_string(&self, name: &str) -> Option<String> {
        self.maybe_flag(name,|v| v.as_string())
    }

    /// get flag as an integer
    pub fn maybe_integer(&self, name: &str) -> Option<i32> {
        self.maybe_flag(name,|v| v.as_int())
    }

    /// get flag as a float
    pub fn maybe_float(&self, name: &str) -> Option<f32> {
        self.maybe_flag(name,|v| v.as_float())
    }


    fn get_flag_value (&self, name: &str) -> &Value {
        if let Some(val_ref) = self.maybe_flag_value(name) {
            val_ref
        } else {
            self.bad_flag(name,"not found")
        }
    }

    fn bad_flag (&self, tname: &str, msg: &str) -> ! {
        self.quit(&format!("flag '{}': {}",tname,msg))
    }

    fn get_flag<T, F: Fn(&Value) -> Result<T>> (&self, name: &str, extract: F) -> T {
        match extract(self.get_flag_value(name)) {
            Ok(v) => v,
            Err(e) => self.bad_flag(name,e.description())
        }
    }

    /// get flag as a string, quitting otherwise.
    pub fn get_string(&self, name: &str) -> String {
        self.get_flag(name,|v| v.as_string())
    }

    /// get flag as an integer, quitting otherwise.
    pub fn get_integer(&self, name: &str) -> i32 {
        self.get_flag(name,|v| v.as_int())
    }

    /// get flag as a float, quitting otherwise.
    pub fn get_float(&self, name: &str) -> f32 {
        self.get_flag(name,|v| v.as_float())
    }

    /// get flag as a bool, quitting otherwise.
    pub fn get_bool(&self, name: &str) -> bool {
        self.get_flag(name,|v| v.as_bool())
    }

    /// get flag as a file for reading, quitting otherwise.
    pub fn get_infile(&self, name: &str) -> Box<Read> {
        self.get_flag(name,|v| v.as_infile())
    }

    /// get flag as a file for writing, quitting otherwise.
    pub fn get_outfile(&self, name: &str) -> Box<Write> {
        self.get_flag(name,|v| v.as_outfile())
    }

    fn get_boxed_array(&self, name: &str, kind: &str) -> &Vec<Box<Value>> {
        match self.get_flag_value(name).as_array() {
            Err(e) => self.bad_flag(name,e.description()),
            Ok(arr) => {
                // empty array matches all types
                if arr.len() == 0 { return arr; }
                // otherwise check the type of the first element
                let ref v = *(arr[0]);
                let tname = v.type_of().short_name();
                if tname == kind {arr} else  { self.bad_flag(name,kind) }
            }
        }
    }


    fn get_array<T,F>(&self, name: &str, kind: &str, extract: F) -> Vec<T>
    where T: Sized, F: Fn(&Box<Value>)->Result<T> {
        let va = self.get_boxed_array(name,kind);
        let mut res = Vec::new();
        for v in va {
            let n = extract(v).unwrap();
            res.push(n);
        }
        res
    }

    /// get multiple flag as an array of strings, quitting otherwise.
    pub fn get_strings(&self, name: &str) -> Vec<String> {
        self.get_array(name,"string",|b| b.as_string())
    }

    /// get multiple flag as an array of integers, quitting otherwise.
    pub fn get_integers(&self, name: &str) -> Vec<i32> {
        self.get_array(name,"integer",|b| b.as_int())
    }

    /// get multiple flag as an array of floats, quitting otherwise.
    pub fn get_floats(&self, name: &str) -> Vec<f32> {
        self.get_array(name,"float",|b| b.as_float())
    }

}

/// parse the command-line specification and use it
/// to parse the program's command line args.
/// As before, quits on any error.
pub fn parse_args(s: &str) -> Args {
    let mut res = Args::new(s);
    res.parse();
    res
}


#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE: &'static str = "
Testing Lapp
  -v, --verbose verbose flag
  -k   arb flag
  -o, --output (default 'stdout')
  -p   (integer...)
  -I, --include... (string)
  <in> (string)
  <out> (string...)
";

    fn arg_strings(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    fn empty_strings() -> Vec<String> {
        Vec::new()
    }

    struct SimpleTest {
        verbose: bool,
        k: bool,
        output: String,
        p: Vec<i32>,
        include: Vec<String>,
        out: Vec<String>
    }

    impl SimpleTest {
        fn new(test_args: &[&str]) -> SimpleTest {
            let mut args = Args::new(SIMPLE);
            args.parse_spec().expect("spec failed");
            args.parse_command_line(arg_strings(test_args)).expect("scan failed");
            SimpleTest {
                verbose: args.get_bool("verbose"),
                k: args.get_bool("k"),
                output: args.get_string("output"),
                p: args.get_integers("p"),
                include: args.get_strings("include"),
                out: args.get_strings("out")
            }
        }
    }

    #[test]
    fn test_simple_just_out() {
        let res = SimpleTest::new(&["boo","hello"]);
        assert_eq!(res.verbose,false);
        assert_eq!(res.k,false);
        assert_eq!(res.output,"stdout");
        assert_eq!(res.p,&[]);
        assert_eq!(res.out,&["hello"]);
    }

    #[test]
    fn test_simple_bool_flags() {
        let res = SimpleTest::new(&["boo","-vk","hello"]);
        assert_eq!(res.verbose,true);
        assert_eq!(res.k,true);
        assert_eq!(res.output,"stdout");
        assert_eq!(res.p,&[]);
        assert_eq!(res.out,&["hello"]);
    }

    #[test]
    fn test_simple_array_flag() {
        let res = SimpleTest::new(&["boo","-p","10 20 30","hello"]);
        assert_eq!(res.verbose,false);
        assert_eq!(res.k,false);
        assert_eq!(res.output,"stdout");
        assert_eq!(res.p,&[10,20,30]);
        assert_eq!(res.out,&["hello"]);
    }

    #[test]
    fn test_simple_multiple_positional_args() {
        let res = SimpleTest::new(&["boo","hello","baggins","--","--frodo"]);
        assert_eq!(res.verbose,false);
        assert_eq!(res.k,false);
        assert_eq!(res.output,"stdout");
        assert_eq!(res.p,&[]);
        assert_eq!(res.include,empty_strings());
        assert_eq!(res.out,&["hello","baggins","--frodo"]);
    }

    #[test]
    fn test_simple_multiple_flags() {
        let res = SimpleTest::new(&["boo","-I.","-I..","--include","lib","hello"]);
        assert_eq!(res.verbose,false);
        assert_eq!(res.k,false);
        assert_eq!(res.output,"stdout");
        assert_eq!(res.p,&[]);
        assert_eq!(res.include,&[".","..","lib"]);
        assert_eq!(res.out,&["hello"]);
    }


}

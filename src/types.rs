// Type and Value structs; errors

use std::error::Error;
use std::fmt;
use std::result;
use std::string;
use std::io;
use std::fs::File;
use std::io::prelude::*;

#[derive(Debug)]
pub struct LappError(pub String);

impl fmt::Display for LappError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}",self.0)
    }
}

impl Error for LappError {
    fn description(&self) -> &str {
        &self.0
    }
}

pub type Result<T> = result::Result<T,LappError>;


pub fn error<T, M: string::ToString>(msg: M) -> Result<T> {
    Err(LappError(msg.to_string()))
}


// the flag types
#[derive(Debug, PartialEq)]
pub enum Type {
    Str,
    Int,
    Float,
    Bool,
    FileIn,
    FileOut,
    None,
    Arr(Box<Type>)
}

impl Default for Type {
    fn default() -> Type { Type::None }
}


impl Type {
    pub fn from_name(s: &str) -> Result<Type> {
        match s {
        "string" => Ok(Type::Str),
        "integer" => Ok(Type::Int),
        "float" => Ok(Type::Float),
        "bool" => Ok(Type::Bool),
        "infile" => Ok(Type::FileIn),
        "outfile" => Ok(Type::FileOut),
        _ => error(format!("not a known type {}",s))
        }
    }

    pub fn array_type(&self) -> Option<&Type> {
        // &**bt means (from R to L) deref bt, unbox type, return reference
        match *self {Type::Arr(ref bt) => Some(&**bt), _ => None}
    }

    pub fn create_empty_array(self) -> Type {
        Type::Arr(Box::new(self))
    }

    pub fn short_name(&self) -> String {
        (match *self {
         Type::Str => "string",
         Type::Int => "integer",
         Type::Float => "float",
         Type::Bool => "bool",
         Type::FileIn => "infile",
         Type::FileOut => "outfile",
         Type::Arr(_) => "array",
         _ => "bad"
        }).to_string()
    }

    pub fn rust_name(&self, multiple: bool) -> String {
        let mut res = match *self {
            Type::Bool => "bool".into(),
            Type::Float => "f32".into(),
            Type::Int => "i32".into(),
            Type::Str => "String".into(),
            Type::FileIn => "Box<Read>".into(),
            Type::FileOut => "Box<Write>".into(),
            Type::Arr(ref t) => format!("Vec<{}>",t.rust_name(false)),
            _ => "bad".into()
        };
        if multiple {
            res = format!("Vec<{}>",res);
        }
        res
    }

    pub fn parse_string(&self, s: &str) -> Result<Value> {
        match *self {
        Type::Str => Ok(Value::Str(s.to_string())),
        Type::Int =>
            match s.parse::<i32>() {
                Ok(n) => Ok(Value::Int(n)),
                Err(e) => error(format!("can't convert '{}' to integer - {}",s,e.description()))
            },
        Type::Float =>
            match s.parse::<f32>() {
                Ok(v) => Ok(Value::Float(v)),
                Err(e) => error(format!("can't convert '{}' to float - {}",s,e.description()))
            },
        Type::FileIn => Ok(Value::FileIn(s.to_string())),
        Type::FileOut => Ok(Value::FileOut(s.to_string())),
        Type::Arr(ref bt) => {
            // multiple values either space or comma separated
            let parts: Vec<_> = if s.find(',').is_some() {
                s.split(',').collect()
            } else {
                s.split_whitespace().collect()
            };
            let mut res = Vec::new();
            for part in parts {
                let v = bt.parse_string(part)?;
                res.push(Box::new(v));
            }
            Ok(Value::Arr(res))
          }
        _ => error(format!("can't convert '{}' to {:?}",s,self))
        }
    }

}

// and values...
#[derive(Debug, Clone)]
pub enum Value {
    Str(String),
    Int(i32),
    Float(f32),
    Bool(bool),
    FileIn(String),
    FileOut(String),
    None,
    Arr(Vec<Box<Value>>)
}

impl Default for Value {
    fn default() -> Value { Value::None }
}

impl Value {
    pub fn as_string(&self) -> Result<String> {
        match *self { Value::Str(ref s) => Ok(s.clone()), _ => error("not a string") }
    }

    pub fn as_int(&self) -> Result<i32> {
        match *self { Value::Int(n) => Ok(n), _ => error("not an integer" )}
    }

    pub fn as_float(&self) -> Result<f32> {
        match *self { Value::Float(x) => Ok(x), _ => error("not a float") }
    }

    pub fn as_bool(&self) -> Result<bool> {
        match *self { Value::Bool(b) => Ok(b), _ => error("not a boolean") }
    }

    pub fn as_infile(&self) -> Result<Box<Read>> {
        match *self {
             Value::FileIn(ref s) => {
                if s == "stdin" { return Ok(Box::new(io::stdin())); }
                match File::open(s) {
                    Ok(f) => Ok(Box::new(f)),
                    Err(e) => error(format!("can't open '{}' for reading: {}",s, e.description()))
                }
             },
              _ => error("not a infile")
        }
    }

    pub fn as_outfile(&self) -> Result<Box<Write>> {
        match *self {
             Value::FileOut(ref s) => {
                if s == "stdout" { return Ok(Box::new(io::stdout())); }
                match File::create(s) {
                    Ok(f) => Ok(Box::new(f)),
                    Err(e) => error(format!("can't open '{}' for writing: {}",s, e.description()))
                }
             },
              _ => error("not an outfile")
        }
    }


    pub fn as_array(&self) -> Result<&Vec<Box<Value>>> {
        match *self { Value::Arr(ref vi) => Ok(vi), _ => error("not an array") }
    }

    pub fn type_of(&self) -> Type {
        match *self {
        Value::Str(_) => Type::Str,
        Value::Int(_) => Type::Int,
        Value::Float(_) => Type::Float,
        Value::Bool(_) => Type::Bool,
        Value::FileIn(_) => Type::FileIn,
        Value::FileOut(_) => Type::FileOut,
        Value::None => Type::None,
        // watch out here...
        Value::Arr(ref v) => (*v[0]).type_of()
        }
    }

    pub fn from_value (val: &str) -> Result<Value> {
        let firstc = val.chars().next().unwrap();
        if firstc.is_digit(10) {
            let t = if val.find('.').is_some() { Type::Float } else { Type::Int };
            t.parse_string(val)
        } else
        if firstc == '\'' { // strip quotes
            Ok(Value::Str((&val[1..(val.len()-1)]).to_string()))
        } else
        if val == "stdin" {
            Ok(Value::FileIn("stdin".into()))
        } else
        if val == "stdout" {
            Ok(Value::FileOut("stdout".into()))
        } else {
            Ok(Value::Str(val.to_string()))
        }
    }

    pub fn empty_array() -> Value {
        let empty: Vec<Box<Value>> = Vec::new();
        Value::Arr(empty)
    }

}


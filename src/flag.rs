// Flag struct

use super::types::*;

#[derive(Default)]
pub struct Flag {
    pub long: String,
    pub short: char,
    pub vtype: Type,
    pub value: Value,
    pub defval: Value,
    pub is_set: bool,
    pub is_multiple: bool,
    pub pos: usize,
    pub help: String,
    pub constraint: Option<Box< Fn(Value) -> Result<Value> >>,
}


impl Flag {
    pub fn set_value_from_string(&mut self, arg: &str) -> Result<()> {
        let mut v = self.vtype.parse_string(arg)?;
        // there may be a constrait on this flag value
        if let Some(ref constraint) = self.constraint {
            v = constraint(v)?;
        }
        self.set_value(v)?;
        Ok(())
    }

    pub fn set_range_constraint(&mut self, b1: &str, b2: &str) -> Result<()> {
        let b1 = Value::from_value(b1)?;
        let b2 = Value::from_value(b2)?;
        if b1.type_of() != b2.type_of() {
            return error("range values must be same type");
        }
        let tn = b1.type_of().short_name();
        if ! (tn == "integer" || tn == "float") {
            return error("range values must be integer or float");
        }
        self.vtype = b1.type_of();

        if tn == "integer" {
            let i1 = b1.as_int().unwrap();
            let i2 = b2.as_int().unwrap();
            let msg = format!("{} {}..{}",self.long,i1,i2);
            self.constraint = Some(Box::new(
                move |v| {
                    let i = v.as_int().unwrap();
                    if i < i1 || i > i2 {
                        return error(format!("{} out of range",msg));
                    }
                    Ok(Value::Int(i))
                }
            ));
        } else {
            let x1 = b1.as_float().unwrap();
            let x2 = b2.as_float().unwrap();
            let msg = format!("{} {}..{}",self.long,x1,x2);
            self.constraint = Some(Box::new(
                move |v| {
                    let x = v.as_float().unwrap();
                    if x < x1 || x > x2 {
                        return error(format!("{} out of range",msg));
                    }
                    Ok(Value::Float(x))
                }
            ));
        }
        Ok(())
    }

    pub fn set_value(&mut self, v: Value) -> Result<()> {
        if self.is_set && ! self.is_multiple {
            return error(format!("flag already specified {}",self.long));
        }
        self.is_set = true;
        if ! self.is_multiple {
            self.value = v;
        } else {
            if let Value::Arr(ref mut arr) = self.value {
                arr.push(Box::new(v));
            }
        }
        Ok(())
    }

    // When checking any missing flags after scanning args, insist
    // that they have default values - otherwise they are 'required'.
    // (Array values may be empty tho)
    pub fn check(&mut self) -> Result<()> {
        if ! self.is_set {
            if let Value::None = self.defval  {
                if let Type::Arr(_) = self.vtype {
                } else
                if ! self.is_multiple {
                    return error(format!("required flag {}",self.long));
                }
            } else {
                self.value = self.defval.clone();
            }
        }
        Ok(())
    }

    pub fn rust_name(&self) -> String {
        // long name may need massaging to become a Rust variable name
        // The result must be snake_case to keep compiler happy!
        let mut name = self.long.replace('-',"_").to_lowercase().to_string();
        let firstc = name.chars().nth(0).unwrap();
        if firstc.is_digit(10) || firstc == '_' {
            name = format!("c_{}",name);
        }
        name
    }

    pub fn rust_type(&self) -> String {
        self.vtype.rust_name(self.is_multiple)
    }

    pub fn getter_name(&self) -> String {
        let mut tname = self.vtype.short_name();
        // Is this an array flag? Two possibilities - the type is an array,
        // or our multiple flag is set.
        let maybe_array = self.vtype.array_type();
        if maybe_array.is_some() {
            tname = maybe_array.unwrap().short_name() + "s";
        } else
        if self.is_multiple {
            tname.push('s');
        }
        format!("args.get_{}(\"{}\")",tname,self.long)
    }



}

use super::LappError;

pub fn skipws(slice: &str) -> &str {
    let nxt = slice.find(|c: char| ! c.is_whitespace()).unwrap_or(slice.len());
    &slice[nxt..]
}

pub fn starts_with(pslice: &mut &str, start: &str) -> bool {
    if pslice.starts_with(start) {
        *pslice = &pslice[start.len()..];
        true
    } else {
        false
    }
}

pub fn ends_with(pslice: &mut &str, start: &str) -> bool {
    if pslice.ends_with(start) {
        *pslice = &pslice[0..(pslice.len() - start.len())];
        true
    } else {
        false
    }
}

pub fn grab_upto(pslice: &mut &str, sub: &str) -> Result<String,LappError> {
    if let Some(idx) = pslice.find(sub) {
        let s = (&pslice[0..idx].trim()).to_string();
        *pslice = &pslice[idx+sub.len()..];
        Ok(s)
    } else {
        Err(LappError(format!("cannot find end {:?}",sub)))
    }
}

pub fn split_with<'a>(slice: &'a str, needle: &str) -> Option<(&'a str,&'a str)> {
    if let Some(idx) = slice.find(needle) {
        Some((
            &slice[0..idx], &slice[idx+needle.len()..]
        ))
    } else {
        None
    }
}

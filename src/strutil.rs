use super::LappError;

pub fn skipws(slice: &str) -> &str {
    let nxt = slice.find(|c: char| ! c.is_whitespace()).unwrap_or(slice.len());
    &slice[nxt..]
}

pub fn grab_word<'a>(pslice: &mut &str) -> String {
    let nxt = pslice.find(|c: char| c.is_whitespace()).unwrap_or(pslice.len());
    let word = (&pslice[0..nxt]).to_string();
    *pslice = skipws(&pslice[nxt..]);
    word
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

pub fn dedent(s: &str) -> String {
    let mut lines = s.lines();
    let mut res = String::new();
    let mut idx = None;
    while let Some(line) = lines.next() {
        if let Some(pos) = line.chars().position(|c| ! c.is_whitespace()) {
            idx = Some(pos);
            res += &line[pos..];
            res.push('\n');
            break;
        }
    }
    if let Some(pos) = idx {
        while let Some(line) = lines.next() {
            res += if line.len() >= pos { &line[pos..] } else { line };
            res.push('\n');
        }
    }
    res
}


pub fn join_labels<'a, I: IntoIterator<Item = Option<&'a str>>>(opts: I, sep: char) -> Option<String> {
  let mut s = String::new();
  let mut o = opts.into_iter();
  loop {
    let v = match o.next() {
      Some(v) => v,
      None    => break,
    };
    if let Some(v) = v {
      if s.len() > 0 { s.push(sep); }
      s.push_str(v);
    }
  }
  if s.len() > 0 {
    Some(s)
  } else{
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_join_labels() {
    assert_eq!(join_labels(vec![], '/'), None);
    assert_eq!(join_labels(vec![None], '/'), None);
    assert_eq!(join_labels(vec![None, None], '/'), None);
    assert_eq!(join_labels(vec![None, Some("Hi"), None], '/'), Some("Hi".to_owned()));
    assert_eq!(join_labels(vec![Some("Hi"), Some("there"), Some("palzo")], '/'), Some("Hi/there/palzo".to_owned()));
  }

}

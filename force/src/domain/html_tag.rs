use strsim::jaro_winkler;
use url::Url;

#[derive(Debug, PartialEq, Clone)]
pub enum HtmlTag {
    ATag(String),
    H3Tag(String),
    SpanTag(String),
    NextPageATag(String),
}

pub fn extract_founder_name(tag: HtmlTag) -> Option<String> {
    match tag {
        HtmlTag::SpanTag(t) => match t.strip_prefix("LinkedIn Â· ") {
            Some(right_word) => {
                let right_word_original = right_word.to_string();

                let result = match right_word.split(",").collect::<Vec<&str>>().as_slice() {
                    [name, ..] => name.to_string(),
                    _ => right_word_original,
                };

                let result = match result.contains("Dr.") {
                    true => result.strip_prefix("Dr.").unwrap().trim().to_string(),
                    false => result,
                };
                let result = match result.contains("Dr") {
                    true => result.strip_prefix("Dr").unwrap().trim().to_string(),
                    false => result,
                };

                Some(result)
            }
            None => None,
        },
        HtmlTag::H3Tag(content) => {
            /*
             Match with both in lowercase
             1. Split by "'s Post -" and get content before the split
             3. Split by "on LinkedIn" and get content before the split
             4. Split by "posted on" and get content before the split
             2. Split by "-" and get content before the split
             5. Split by "|" and get content before the split
            */
            let strategies = [
                "'s Post -",
                "posted on",
                "on LinkedIn",
                "en LinkedIn",
                "auf LinkedIn",
                "sur LinkedIn",
                "-",
                "–", // I know, this is a different character
                "|",
            ];

            let strategies: Vec<String> = strategies.iter().map(|st| st.to_lowercase()).collect();
            let content = content.to_lowercase();

            strategies
                .iter()
                .filter_map(|st| {
                    content
                        .split_once(st)
                        .map(|parts| parts.0.trim().to_string())
                })
                .next()
        }
        _ => None,
    }
}

pub fn extract_domain(tag: HtmlTag) -> Option<String> {
    match tag {
        HtmlTag::ATag(content) => match content.strip_prefix("/url?q=") {
            Some(url) => match Url::parse(url) {
                Ok(parsed_url) => match parsed_url.host_str() {
                    Some("support.google.com") => None,
                    Some("www.google.com") => None,
                    Some("accounts.google.com") => None,
                    Some("policies.google.com") => None,
                    Some("www.amazon.com") => None,
                    Some("") => None,
                    None => None,
                    Some(any_host) => {
                        if any_host.contains("google.com") {
                            None
                        } else {
                            match any_host.strip_prefix("www.") {
                                Some(h) => Some(h.to_lowercase()),
                                None => Some(any_host.to_lowercase()),
                            }
                        }
                    }
                },
                Err(_) => None,
            },
            None => None,
        },
        _ => None,
    }
}

pub fn extract_company_domain(company_name: &str, tags: Vec<String>) -> String {
    tags.into_iter()
        .max_by(|a, b| {
            jaro_winkler(company_name, a)
                .partial_cmp(&jaro_winkler(company_name, b))
                .unwrap()
        })
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::extract_company_domain;

    #[test]
    fn extract_company_domain_valid() {
        let company_name = "Google Company";
        let tags = vec![
            "friends.com".to_string(),
            "goog.com".to_string(),
            "google.com".to_string(),
            "google.us".to_string(),
            "fb.pk".to_string(),
        ];
        let result = extract_company_domain(company_name, tags);

        assert_eq!(result, "google.com");
    }
}

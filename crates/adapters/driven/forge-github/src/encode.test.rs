use super::*;

// RFC 4648 section 10.
#[test]
fn test_base64_is_the_rfc_4648_encoding_with_padding() {
    for (plain, encoded) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
        ("# Example Fragment\n", "IyBFeGFtcGxlIEZyYWdtZW50Cg=="),
    ] {
        assert_eq!(base64(plain.as_bytes()), encoded, "{plain:?}");
    }
}

#[test]
fn test_base64_carries_any_byte_not_only_text() {
    assert_eq!(base64(&[0, 255, 254, 128]), "AP/+gA==");
    assert_eq!(base64(&[251, 255, 191]), "+/+/");
    let every: Vec<u8> = (0..=255).collect();
    let encoded = base64(&every);
    assert_eq!(encoded.len(), 344);
    assert!(encoded.ends_with("/P3+/w=="), "{encoded}");
    assert!(encoded.starts_with("AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8g"));
}

#[test]
fn test_a_path_keeps_its_slashes_and_unreserved_characters_and_encodes_the_rest() {
    assert_eq!(path("release/1.x"), "release/1.x");
    assert_eq!(path("a-b_c.d~e"), "a-b_c.d~e");
    assert_eq!(path("a b"), "a%20b");
    assert_eq!(path("x?y#z"), "x%3Fy%23z");
    assert_eq!(path("100%"), "100%25");
    assert_eq!(path("caf\u{e9}"), "caf%C3%A9");
    assert_eq!(path(""), "");
}

//! Tests queue name handling, without testing the OS.

use std::io::ErrorKind;
use std::ffi::{CStr, CString};

extern crate posixmq;
use posixmq::{PosixMq, OpenOptions, unlink, unlink_c};

#[test]
fn checks_for_nul_in_short_names() {
    assert_eq!(unlink("/trailing\0").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create("/trailing\0").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(unlink("trailing\0").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create("trailing\0").unwrap_err().kind(), ErrorKind::InvalidInput);

    assert_eq!(unlink("/in\0between").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create("/in\0between").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(unlink("in\0between").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create("in\0between").unwrap_err().kind(), ErrorKind::InvalidInput);

    assert_eq!(unlink("/\0first").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create("/\0first").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(unlink("\0first").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create("\0first").unwrap_err().kind(), ErrorKind::InvalidInput);
}

#[test]
fn checks_for_nul_in_long_names() {
    let mut long = [b'w'; 100];
    long[0] = b'/';

    long[99] = b'\0';
    assert_eq!(unlink(&long[..]).unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create(&long[..]).unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(unlink(&long[1..]).unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create(&long[1..]).unwrap_err().kind(), ErrorKind::InvalidInput);
    long[99] = b'w';

    long[50] = b'\0';
    assert_eq!(unlink(&long[..]).unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create(&long[..]).unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(unlink(&long[1..]).unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create(&long[1..]).unwrap_err().kind(), ErrorKind::InvalidInput);
    long[50] = b'w';

    long[1] = b'\0';
    assert_eq!(unlink(&long[..]).unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create(&long[..]).unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(unlink(&long[1..]).unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create(&long[1..]).unwrap_err().kind(), ErrorKind::InvalidInput);
    long[1] = b'w';
}

#[test]
fn create_long_unlink_cstr() {
    let mut long = [b'C'; 100];
    long[0] = b'/';
    long[99] = b'\0';

    OpenOptions::readwrite()
        .create_new()
        .open(&long[1..99])
        .expect("create new queue with long name");
    unlink_c(CStr::from_bytes_with_nul(&long).unwrap())
        .expect("delete queue with long name");
}

#[test]
fn create_short_unlink_cstr() {
    let mut excl = OpenOptions::readwrite();
    excl.create_new();

    excl.open("short").expect("create new queue short");
    assert_eq!(
        excl.open("/short").expect_err("try to re-create /short").kind(),
        ErrorKind::AlreadyExists,
    );
    unlink_c(CStr::from_bytes_with_nul(b"/short\0").unwrap())
        .expect("delete /short");
}

fn test_normalization(name_with_slash: &str) {
    let proper_c_string = CString::new(name_with_slash).expect("queue name contains \\0");

    // trailing NUL
    assert_eq!(
        PosixMq::open(proper_c_string.as_bytes_with_nul()).expect_err(
            &format!("{}\\0 ({} bytes) is rejected", name_with_slash, name_with_slash.len()+1)
        ).kind(),
        ErrorKind::InvalidInput,
        "opening {}\\0 ({} bytes) is rejected", name_with_slash, name_with_slash.len()+1
    );
    assert_eq!(
        unlink(proper_c_string.as_bytes_with_nul()).unwrap_err().kind(),
        ErrorKind::InvalidInput,
        "deleting {}\\0 ({} bytes) is rejected", name_with_slash, name_with_slash.len()+1
    );
    assert_eq!(
        PosixMq::open(&proper_c_string.as_bytes_with_nul()[1..]).expect_err(
            &format!("{}\\0 ({} bytes) is rejected", &name_with_slash[1..], name_with_slash.len())
        ).kind(),
        ErrorKind::InvalidInput,
        "opening {}\\0 ({} bytes) is rejected", &name_with_slash[1..], name_with_slash.len()
    );
    assert_eq!(
        unlink(&proper_c_string.as_bytes_with_nul()[1..]).unwrap_err().kind(),
        ErrorKind::InvalidInput,
        "deleting {}\\0 ({} bytes) is rejected", &name_with_slash[1..], name_with_slash.len()
    );

    OpenOptions::readwrite().create_new().open_c(&proper_c_string).expect(
        &format!("create new {} ({} bytes) via CStr", name_with_slash, name_with_slash.len())
    );
    assert_eq!(
        OpenOptions::readwrite().create_new().open(name_with_slash).expect_err(
            &format!("re-create {} ({} bytes)", name_with_slash, name_with_slash.len())
        ).kind(),
        ErrorKind::AlreadyExists,
        "{} ({} bytes) already exists", name_with_slash, name_with_slash.len()
    );
    assert_eq!(
        OpenOptions::readwrite().create_new().open(&name_with_slash[1..]).expect_err(
            &format!("re-create {} ({} bytes)", &name_with_slash[1..], name_with_slash.len()-1)
        ).kind(),
        ErrorKind::AlreadyExists,
        "{} ({} bytes) already exists", &name_with_slash[1..], name_with_slash.len()-1
    );
    unlink(&name_with_slash[1..]).expect(
        &format!("delete {} ({} bytes)", &name_with_slash[1..], name_with_slash.len()-1)
    );
}

#[test]
fn short() {
    test_normalization("/short");
}

#[test]
fn long() {
    let mut long = [b'L'; 100];
    long[0] = b'/';
    test_normalization(std::str::from_utf8(&long).unwrap());
}

#[test]
fn edge_between_short_and_long() {
    for len_with_slash in 25..=40 {
        let mut name = String::from("/name_");
        name.extend((b'A'..=b'Z').take(len_with_slash-6).map(|c| c as char ));
        test_normalization(&name);
    }
}

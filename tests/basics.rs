//! Tests of the portable core features.

use std::io::ErrorKind;

extern crate posixmq;
use posixmq::{PosixMq, OpenOptions, Attributes, unlink, name_from_bytes};

#[test]
fn name_with_nul() {
    assert_eq!(unlink("/foo\0").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(PosixMq::create("/foo\0").unwrap_err().kind(), ErrorKind::InvalidInput);
}

#[test]
fn nonexistant() {
    let _ = unlink(b"/404"); // in case it already exists
    assert_eq!(unlink("/404").unwrap_err().kind(), ErrorKind::NotFound);
    assert_eq!(PosixMq::open("/404").unwrap_err().kind(), ErrorKind::NotFound);
}

#[test]
fn open_custom_capacities() {
    let mq = OpenOptions::readonly()
        .capacity(2)
        .max_msg_len(100)
        .create()
        .open(b"/custom_capacities")
        .unwrap();
    let _ = posixmq::unlink(b"/custom_capacities");
    assert_eq!(mq.attributes(), Attributes {
        capacity: 2,
        max_msg_len: 100,
        current_messages: 0,
        nonblocking: false,
    });
}

#[test]
fn create_and_remove() {
    let mq = OpenOptions::readwrite().create_new().open("/flash");
    mq.expect("cannot create");
    unlink("/flash").expect("cannot unlink");
    assert_eq!(PosixMq::open("/flash").unwrap_err().kind(), ErrorKind::NotFound);
}


#[test]
fn is_not_nonblocking() {
    let mq = PosixMq::create("/is_not_nonblocking").unwrap();
    let _ = posixmq::unlink("/is_not_nonblocking");
    assert!(!mq.is_nonblocking());
    // TODO test that send and receive blocks?
}

#[test]
fn is_nonblocking() {
    let mq = OpenOptions::readwrite()
        .nonblocking()
        .capacity(1)
        .max_msg_len(1)
        .create()
        .open("/is_nonblocking")
        .unwrap();
    let _ = posixmq::unlink("/is_nonblocking");

    assert!(mq.is_nonblocking());
    assert_eq!(mq.receive(&mut[0]).unwrap_err().kind(), ErrorKind::WouldBlock);
    mq.send(5, b"e").unwrap();
    assert_eq!(mq.send(6, b"f").unwrap_err().kind(), ErrorKind::WouldBlock);
}

#[test]
fn change_nonblocking() {
    let mq = PosixMq::create("/change_nonblocking").unwrap();
    let _ = posixmq::unlink("/change_nonblocking");
    mq.set_nonblocking(true).unwrap();
    assert!(mq.is_nonblocking());
    assert_eq!(mq.receive(&mut[0; 8192]).unwrap_err().kind(), ErrorKind::WouldBlock);
    mq.set_nonblocking(false).unwrap();
    assert!(!mq.is_nonblocking());
}


#[test]
fn send_errors() {
    let _ = unlink("send");
    let nb = OpenOptions::writeonly()
        .nonblocking()
        .create()
        .max_msg_len(1)
        .capacity(2)
        .open("send")
        .unwrap();
    assert_eq!(nb.send(0, b"too long").unwrap_err().kind(), ErrorKind::Other);

    let bl = OpenOptions::readwrite().open("send").unwrap();
    assert_eq!(bl.send(!0, b"f").unwrap_err().kind(), ErrorKind::InvalidInput);

    nb.send(9, b"a").expect("nonblocking send \"a\"");
    if cfg!(target_os="netbsd") || cfg!(target_os="dragonfly") {// doesn't allow empty messages
        bl.send(0, b"b").expect("blocking send \"b\"");
    } else {
        bl.send(0, b"").expect("blocking send empty");
    }
    assert_eq!(nb.send(0, b"b").unwrap_err().kind(), ErrorKind::WouldBlock);

    let ro = OpenOptions::readonly().open("send").unwrap();
    assert_eq!(ro.send(0, b"").unwrap_err().kind(), ErrorKind::Other); // opened read-only

    let _ = unlink("send");
}

#[test]
fn receive_errors() {
    let nb = OpenOptions::readonly()
        .nonblocking()
        .create()
        .max_msg_len(1)
        .capacity(2)
        .open("receive")
        .unwrap();
    assert_eq!(nb.receive(&mut[0; 2]).unwrap_err().kind(), ErrorKind::WouldBlock);
    assert_eq!(nb.receive(&mut[]).unwrap_err().kind(), ErrorKind::Other); // buffer too short
    let wo = OpenOptions::writeonly().open("receive").unwrap();
    assert_eq!(wo.receive(&mut[0; 2]).unwrap_err().kind(), ErrorKind::Other); // opened write-only

    let _ = unlink("receive");
}

#[test]
fn send_and_receive() {
    let mq = PosixMq::create(b"/send_and_receive").unwrap();
    let _ = unlink(b"/send_and_receive");

    mq.send(2, b"aaaa").unwrap();
    mq.send(4, b"bbb").unwrap();
    mq.send(1, b"cc").unwrap();
    mq.send(3, b"d").unwrap();

    let mut buf = [0; 8192];
    assert_eq!(mq.receive(&mut buf).unwrap(), (4, 3));
    assert_eq!(&buf[..3], b"bbb");
    assert_eq!(mq.receive(&mut buf).unwrap(), (3, 1));
    assert_eq!(&buf[..3], b"dbb");
    assert_eq!(mq.receive(&mut buf).unwrap(), (2, 4));
    assert_eq!(&buf[..4], b"aaaa");
    assert_eq!(mq.receive(&mut buf).unwrap(), (1, 2));
    assert_eq!(&buf[..4], b"ccaa");
}

#[test]
fn iterators() {
    let mq = OpenOptions::readwrite().nonblocking().create().open("/iterable").unwrap();
    let _ = posixmq::unlink("/iterable");

    for n in 0..8 {
        mq.send(n, n.to_string().as_bytes()).unwrap()
    }
    assert_eq!(mq.iter().next(), Some((7, "7".to_string().into_bytes())));
    for (priority, message) in &mq {
        assert_eq!(String::from_utf8(message).unwrap().parse::<u32>().unwrap(), priority);
    }
    mq.set_nonblocking(false).unwrap();
    for fruit in &["apple", "pear", "watermelon"] {
        mq.send(fruit.len() as u32, fruit.as_bytes()).unwrap();
    }
    let mut iter = mq.into_iter();
    assert_eq!(iter.next(), Some((10, b"watermelon".to_vec())));
    assert_eq!(iter.next(), Some((5, b"apple".to_vec())));
    assert_eq!(iter.next(), Some((4, b"pear".to_vec())));
}

#[test]
#[should_panic]
fn iterator_panics_if_writeonly() {
    let mq = OpenOptions::writeonly().create().open("writeonly").unwrap();
    let _ = unlink("writeonly");
    for (_, _) in mq {

    }
}


#[test]
fn name_from_bytes_normal() {
    use std::borrow::Cow;

    match name_from_bytes("/good\0") {
        Cow::Borrowed(cstr) => assert_eq!(cstr.to_bytes(), b"/good"),
        _ => panic!("should not allocate here"),
    }
    match name_from_bytes(b"/bad") {
        Cow::Owned(cstring) => assert_eq!(cstring.to_bytes(), b"/bad"),
        _ => panic!("should allocate for missing \\0"),
    }
    match name_from_bytes("bad\0") {
        Cow::Owned(cstring) => assert_eq!(cstring.to_bytes(), b"/bad"),
        _ => panic!("should allocate for missing slash"),
    }
    match name_from_bytes("doubly_bad") {
        Cow::Owned(cstring) => assert_eq!(cstring.to_bytes(), b"/doubly_bad"),
        _ => panic!("should allocate for doubly bad"),
    }
    match name_from_bytes(b"") {
        Cow::Owned(cstring) => assert_eq!(cstring.to_bytes(), b"/"),
        _ => panic!("should allocate for empty slice"),
    }
}

#[test]
#[should_panic]
fn name_from_bytes_interior_nul() {
    name_from_bytes("/good\0shit\0");
}


#[test]
#[ignore] // racy
fn drop_closes() {
    let mq = PosixMq::create("/close").unwrap();
    let _ = unlink("/close");
    let mqd = mq.as_raw_mqd();
    let fake_clone = unsafe { PosixMq::from_raw_mqd(mqd) };
    // cast to usize because pointers cannot be compared directly
    assert_eq!(fake_clone.as_raw_mqd() as usize, mqd as usize);
    fake_clone.send(0, b"b").expect("as_raw_mqd() should not close");
    drop(mq);
    fake_clone.send(0, b"b").expect_err("Drop should have closed the descriptor");
    // also tests that drop ignores errors
}

#[test]
fn into_mqd_doesnt_drop() {
    let mq = PosixMq::create("/raw_mqd").unwrap();
    let _ = unlink("/raw_mqd");
    unsafe {
        let mqd = mq.into_raw_mqd();
        let mq = PosixMq::from_raw_mqd(mqd);
        mq.send(0, b"lorem ipsum").expect("descriptor should remain open");
    }
}


#[test]
fn is_send_and_sync() {
    fn is_send<T:Send>() -> bool {true}
    fn is_sync<T:Sync>() -> bool {true}
    is_send::<PosixMq>();
    is_sync::<PosixMq>();
}

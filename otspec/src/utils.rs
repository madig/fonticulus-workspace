/// Convert an array of bits into an integer
pub fn int_list_to_num(int_list: &[u8]) -> u32 {
    let mut flags = 0;
    for flag in int_list {
        flags |= 1 << flag;
    }
    flags
}

/// Tests if all elements of an iterator have the same content
pub fn is_all_the_same<T, U>(mut iter: T) -> bool
where
    T: Iterator<Item = U>,
    U: PartialEq,
{
    if let Some(first) = iter.next() {
        for n in iter {
            if first != n {
                return false;
            }
        }
    }
    true
}

/// Macro to assist constructing a btreemap from pairs
#[macro_export]
macro_rules! btreemap {
        ($($k:expr => $v:expr),* $(,)?) => {
            std::collections::BTreeMap::<_, _>::from_iter(std::array::IntoIter::new([$(($k, $v),)*]))
        };
    }

/// Macro to assist constructing a hashmap from pairs
#[macro_export]
macro_rules! hashmap {
    ($($k:expr => $v:expr),* $(,)?) => {
        std::collections::HashMap::<_, _>::from_iter(std::array::IntoIter::new([$(($k, $v),)*]))
    };
}

/// Macro to assist constructing a btreeset from items
#[macro_export]
macro_rules! btreeset {
    ($($k:expr),* $(,)?) => {
        std::collections::BTreeSet::<_, >::from_iter(std::array::IntoIter::new([$($k,)*]))
    };
}

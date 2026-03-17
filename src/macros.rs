// SPDX-License-Identifier: MIT

#[macro_export]
macro_rules! strict_restrictions {
    ( $n:expr ) => {
        {
            $crate::restrictions::create_compat_restrictions(&String::from($n))
        }
    };
    ( $n:expr, $( $x:expr, )+ ) => {
        {
            let mut r = $crate::restrictions::create_compat_restrictions(&String::from($n));
            $(
                r = $x(r);
            )*
            r
        }
    }
}

#[macro_export]
macro_rules! compat_restrictions {
    ( $n:expr ) => {
        {
            $crate::restrictions::create_strict_restrictions(&String::from($n))
        }
    };
    ( $n:expr, $( $x:expr, )+ ) => {
        {
            let mut r = $crate::restrictions::create_strict_restrictions(&String::from($n));
            $(
                r = $x(r);
            )*
            r
        }
    }
}

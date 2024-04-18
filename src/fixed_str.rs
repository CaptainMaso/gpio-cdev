use std::ops::Deref;

#[derive(Clone, Copy)]
pub struct FixedStr<const N: usize> {
    s: [u8; N],
}

impl<const N: usize> FixedStr<N> {
    #[inline]
    pub const fn empty() -> Self {
        Self { s: [0; N] }
    }

    #[inline]
    pub fn new(s: &str) -> Result<Self, FixedStrErr> {
        let mut f = Self::empty();
        f.write(s)?;
        Ok(f)
    }

    pub fn from_byte_array(mut bytes: [u8; N]) -> Result<Self, FixedStrErr> {
        let nul = find_nul(&bytes);
        let _ = core::str::from_utf8(&bytes[..nul])?;
        if nul < N {
            bytes[nul..].fill(0);
        }

        Ok(FixedStr { s: bytes })
    }

    pub const fn into_byte_array(self) -> [u8; N] {
        self.s
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FixedStrErr> {
        let len = if let Some(len) = bytes.iter().take(N).position(|c| *c == 0) {
            len
        } else if bytes.len() == N {
            N
        } else {
            return Err(FixedStrErr::CapacityOverflow {
                capacity: N,
                required: bytes.len(),
            });
        };

        let mut s = [0; N];

        s[..len].copy_from_slice(&bytes[..len]);

        let _ = core::str::from_utf8(&s[..len])?;

        Ok(Self { s })
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.s.iter().position(|c| *c == 0).unwrap_or(N)
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.s[0] != 0
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        let l = self.len();
        let s = &self.s[0..l];
        unsafe { std::str::from_utf8_unchecked(s) }
    }

    pub fn write(&mut self, s: &str) -> Result<(), FixedStrErr> {
        let l = self.len();
        let new_len = l + s.len();

        if new_len > N {
            return Err(FixedStrErr::CapacityOverflow {
                capacity: N,
                required: new_len,
            });
        }

        let rem = &mut self.s[l..new_len];

        rem.copy_from_slice(s.as_bytes());
        Ok(())
    }
}

impl<const N: usize> Default for FixedStr<N> {
    #[inline(always)]
    fn default() -> Self {
        Self::empty()
    }
}

impl<const N: usize> std::fmt::Debug for FixedStr<N> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("FixedStr").field(&self.as_str()).finish()
    }
}

impl<const N: usize> std::fmt::Display for FixedStr<N> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(f)
    }
}

impl<const N: usize> AsRef<str> for FixedStr<N> {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<const N: usize> Deref for FixedStr<N> {
    type Target = str;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum FixedStrErr {
    #[error(
        "Exceeded fixed string size: required {required} bytes with only {capacity} available"
    )]
    CapacityOverflow { capacity: usize, required: usize },
    #[error("UTF8 Error")]
    Utf8(#[from] core::str::Utf8Error),
}

impl From<FixedStrErr> for std::io::Error {
    fn from(value: FixedStrErr) -> Self {
        std::io::Error::new(std::io::ErrorKind::InvalidData, value)
    }
}

#[inline]
fn find_nul(s: &[u8]) -> usize {
    let mut rem = s;

    while !rem.is_empty() && rem[0] != 0 {
        rem = &rem[1..];
    }

    s.len() - rem.len()
}

// Opting to use a type alias instead of a new-type.
// This means that we don't need to include VecOps unless
// its methods are absolutely necessary.
// Plus, no need to worry about deref or accessors.
pub type V3<T> = [T; 3];

pub trait V3Ops {
    type Ty: Default + num_traits::real::Real;

    fn add(self, b: Self) -> Self;
    fn sub(self, b: Self) -> Self;
    fn cross(self, b: Self) -> Self;
    fn dot(self, b: Self) -> Self::Ty;
    fn scale(self, s: Self::Ty) -> Self;
    fn mag(self) -> Self::Ty;
    fn normalize(self) -> Self;
    fn angle(self, fst: Self, snd: Self) -> Self::Ty;
}

impl<T: Default + num_traits::real::Real> V3Ops for V3<T> {
    type Ty = T;

    fn add(mut self, b: Self) -> Self {
        self[0] = self[0] + b[0];
        self[1] = self[1] + b[1];
        self[2] = self[2] + b[2];
        self
    }

    fn sub(mut self, b: Self) -> Self {
        self[0] = self[0] - b[0];
        self[1] = self[1] - b[1];
        self[2] = self[2] - b[2];
        self
    }

    fn cross(self, b: Self) -> Self {
        [
            self[1] * b[2] - self[2] * b[1],
            self[2] * b[0] - self[0] * b[2],
            self[0] * b[1] - self[1] * b[0],
        ]
    }

    fn dot(self, b: Self) -> Self::Ty {
        self.into_iter()
            .zip(b)
            .fold(Self::Ty::default(), |dot, (a, b)| dot + a * b)

    }

    fn scale(mut self, s: Self::Ty) -> Self {
        self[0] = self[0] * s;
        self[1] = self[1] * s;
        self[2] = self[2] * s;
        self
    }

    fn mag(self) -> Self::Ty {
        self.iter()
            .fold(Self::Ty::default(), |mag, elem| mag + *elem * *elem).sqrt()
    }

    fn normalize(mut self) -> Self {
        let mag = self.mag();

        self[0] = self[0] / mag;
        self[1] = self[1] / mag;
        self[2] = self[2] / mag;
        self
    }

    // Assumes self is the target point, fst and snd are the other 2
    fn angle(self, fst: Self, snd: Self) -> Self::Ty {
        let ab = fst.sub(self);
        let ac = snd.sub(self);

        (ab.dot(ac) / (ab.mag() * ac.mag())).acos()
    }
}
macro_rules! img_field {
    ($func:ident, $proc:ident, $self:expr, $field:ident , () ) => {
        $self
            .$field
            .$func($proc)
            .map_err(|e| $crate::media::StoreError::chained(stringify!($field), e))
    };
    (load_images, $proc:ident, $self:expr, $field:ident, (optional)) => {
        match &mut $self.$field {
            Some(v) => v
                .load_images($proc)
                .map_err(|e| $crate::media::StoreError::chained(stringify!($field), e)),
            None => Ok(()),
        }
    };
    (store_images, $proc:ident, $self:expr, $field:ident, (optional)) => {
        match &$self.$field {
            Some(v) => v
                .store_images($proc)
                .map_err(|e| $crate::media::StoreError::chained(stringify!($field), e)),
            None => Ok(()),
        }
    };
    ($func:ident, $proc:ident, $self:expr, $field:ident, (no_chain)) => {
        $self.$field.$func($proc)
    };
}
macro_rules! proc_img_fields {
  ($func:ident, $proc:ident, $self:expr, last, {$field:ident : image $spec:tt $(,)? }) => {
      return img_field!($func, $proc, $self, $field, $spec);
  };
  ($func:ident, $proc:ident, $self:expr, other, {$field: ident : image $spec:tt $(,)?}) => {
      img_field!($func, $proc, $self, $field, $spec)?;
  };
  ($func:ident, $proc:ident, $self:expr, $pos:ident, {$field:ident : image $spec:tt, $($other:tt)+}) => {
      img_field!($func, $proc, $self, $field, $spec)?;
      proc_img_fields!($func, $proc, $self, $pos, { $($other)+ });
  };
  ($func:ident, $proc:ident, $self:expr, $pos:ident, {$field:ident : flatten $spec:tt $(,)? }) => {
      proc_img_fields!($func, $proc, $self.$field, $pos, $spec);
  };
  ($func:ident, $proc:ident, $self:expr, $pos:ident, {$field:ident : flatten $spec:tt, $($other:tt)+}) => {
      proc_img_fields!($func, $proc, $self.$field, other, $spec);
      proc_img_fields!($func, $proc, $self, $pos, { $($other)+ });
  };
}
macro_rules! proc_img_ref {
    ($r_set: ident, $self: expr, $field:ident, (optional)) => {
        match &$self.$field {
            Some(v) => v.image_refs($r_set),
            None => (),
        }
    };
    ($r_set:ident, $self:expr, $field:ident, $spec:tt) => {
        $self.$field.image_refs($r_set);
    };
}
macro_rules! proc_img_refs {
  ($r_set:ident, $self:expr, {$(,)?}) => {};
  ($r_set:ident, $self:expr, {$(,)? $field:ident : image $spec:tt $($other:tt)* }) => {
      proc_img_ref!($r_set, $self, $field, $spec);
      proc_img_refs!($r_set, $self, { $($other)* });
  };
  ($r_set:ident, $self:expr, {$(,)? $field:ident : flatten $spec:tt $($other:tt)* }) => {
      proc_img_refs!($r_set, $self.$field, $spec);
      proc_img_refs!($r_set, $self, { $($other)* });
  };
}
macro_rules! has_image {
    ($t:ident $spec:tt) => {
        impl $crate::media::HasImage for $t {
            fn load_images(
                &mut self,
                loader: &mut $crate::media::Loader,
            ) -> Result<(), $crate::media::StoreError> {
                proc_img_fields!(load_images, loader, self, last, $spec);
            }
            fn image_refs<'a, 'b>(&'b self, ref_set: &'a mut $crate::media::RefSet<'b>)
            where
                'b: 'a,
            {
                proc_img_refs!(ref_set, self, $spec);
            }
            fn store_images(
                &self,
                storer: &mut $crate::media::Storer,
            ) -> Result<(), $crate::media::StoreError> {
                proc_img_fields!(store_images, storer, self, last, $spec);
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use openkeychron_guard::{GuardError, require_write_verified};

    use crate::support::v5_max_on_cable;

    #[test]
    fn lets_the_v5_max_iso_on_a_cable_write() {
        let verified = require_write_verified("keymap writes", &v5_max_on_cable()).unwrap();
        assert_eq!(verified.device(), &v5_max_on_cable());
    }

    #[test]
    fn refuses_a_receiver_even_with_a_verified_model_id() {
        let mut receiver = v5_max_on_cable();
        receiver.is_receiver = true;
        let error = require_write_verified("keymap writes", &receiver).unwrap_err();
        assert_eq!(
            error.to_string(),
            "keymap writes require the V5 Max ISO (3434:0951) on a USB cable; receiver and \
             unverified-model writes are refused (got receiver 3434:0951)"
        );
    }

    #[test]
    fn refuses_a_model_not_verified_for_writes() {
        let mut other = v5_max_on_cable();
        other.product_id = 0x0952;
        let error = require_write_verified("macro writes", &other).unwrap_err();
        assert!(
            matches!(error, GuardError::NotWriteVerified { .. }),
            "{error:?}"
        );
    }
}

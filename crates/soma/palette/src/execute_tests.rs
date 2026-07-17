use soma_domain::Confirmation;

use super::confirmation_for;

#[test]
fn confirmed_flag_maps_to_confirmed() {
    assert_eq!(confirmation_for(true), Confirmation::Confirmed);
}

#[test]
fn unconfirmed_flag_maps_to_missing() {
    assert_eq!(confirmation_for(false), Confirmation::Missing);
}

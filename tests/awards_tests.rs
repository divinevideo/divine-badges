use divine_badges::awards::award_catalog;

#[test]
fn award_catalog_contains_three_fixed_creator_awards() {
    let awards = award_catalog();
    assert_eq!(awards.len(), 3);
    assert!(awards
        .iter()
        .any(|award| award.slug == "diviner_of_the_day"));
    assert!(awards
        .iter()
        .any(|award| award.slug == "diviner_of_the_week"));
    assert!(awards
        .iter()
        .any(|award| award.slug == "diviner_of_the_month"));
}

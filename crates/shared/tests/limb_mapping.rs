use bevy::math::{Dir3, Vec3};
use shared::{zombie_hit_damage_ray, BoneWorldPositions, ZombiePart};

/// Each limb gets its own lane in Z so a ray down X can only reach one of them.
fn dummy() -> BoneWorldPositions {
    let lane = |z: f32| [Some(Vec3::new(0.0, 1.0, z)), Some(Vec3::new(0.0, 0.6, z)), Some(Vec3::new(0.0, 0.2, z))];
    BoneWorldPositions {
        head: Some(Vec3::new(0.0, 9.0, 0.0)),
        belly: Some(Vec3::new(0.0, 9.0, 9.0)),
        chest: Some(Vec3::new(0.0, 9.5, 9.0)),
        left_arm: lane(0.0),
        right_arm: lane(2.0),
        left_leg: lane(4.0),
        right_leg: lane(6.0),
    }
}

#[test]
fn a_hit_is_tallied_against_the_limb_it_landed_on() {
    let bones = dummy();
    for (expected, z) in [
        (ZombiePart::LeftArm, 0.0),
        (ZombiePart::RightArm, 2.0),
        (ZombiePart::LeftLeg, 4.0),
        (ZombiePart::RightLeg, 6.0),
    ] {
        let hit = zombie_hit_damage_ray(Vec3::new(-5.0, 0.6, z), Dir3::X, 20.0, &bones)
            .unwrap_or_else(|| panic!("ray at z={z} hit nothing"));
        let (part, _) = hit.limb.unwrap_or_else(|| panic!("ray at z={z} reported no limb"));
        println!("z={z} -> {part:?} (expected {expected:?})");
        assert_eq!(part, expected, "ray at z={z} was tallied against the wrong limb");
    }
}

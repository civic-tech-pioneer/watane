//! Guard contracts for interrupted installer recovery (TECHNICAL 3.6, 3.8 and 8.5).

use crate::support::Fx;
use crate::support::unit::case;

#[test]
fn active_swap_requires_a_checked_destination() {
    let fx = Fx::new();
    let units = fx.units_in_home(&[
        (
            "unchecked",
            r#"
            mkdir -p "$HOME/.local/bin"
            SWAP_ACTIVE=1
            FOLDERS_RESOLVED=
            recovery_allowed "$HOME/.local/bin/watane" "$$"
        "#,
        ),
        (
            "checked",
            r#"
            mkdir -p "$HOME/.local/bin"
            SWAP_ACTIVE=1
            FOLDERS_RESOLVED="resolved .local/bin $HOME_R/.local/bin"
            recovery_allowed "$HOME/.local/bin/watane" "$$"
        "#,
        ),
    ]);
    assert_eq!(case(&units, "unchecked").code, 1);
    assert_eq!(case(&units, "checked").code, 0);
}

#[test]
fn recovery_callers_obey_the_destination_check() {
    let fx = Fx::new();
    // Isolate the callers from the path policy so that each gate is tested
    // independently. All fixtures stay at ordinary paths in the test home.
    let prelude = r#"
        check_home
        set_entries
        bin=$HOME/.local/bin/watane
        mkdir -p "$SHAREW" "${bin%/*}"
        pid=99999999
        pid_gone() { return 0; }
        recovery_allowed() { [ "$1" != "$denied" ]; }
        printf 'watane-receipt 1\nversion 0.1.0\npath %s\n' "$bin" >"$RECEIPT.new.$pid"
        printf 'new %s\nmoved %s\n' "$bin" "$bin" >"$SHAREW/journal.$pid"
    "#;
    let units = fx.units(
        prelude,
        &[],
        &[
            (
                "undo_receipt",
                r#"
                denied=$RECEIPT
                read_journal() { echo 'entered undo journal'; J_OLD=; J_NEW=; }
                undo_swap "$pid"
            "#,
            ),
            (
                "stale_receipt",
                r#"
                denied=$RECEIPT
                : >"$bin.new.$pid"
                clean_stale report
                rm -f "$bin.new.$pid"
            "#,
            ),
            (
                "stale_entry",
                r#"
                denied=$bin
                : >"$bin.new.$pid"
                clean_stale report
                rm -f "$bin.new.$pid"
            "#,
            ),
            (
                "foreign_receipt",
                r#"
                denied=$RECEIPT
                : >"$bin"
                RELEASE=0.1.0
                check_foreign
            "#,
            ),
            (
                "foreign_entry",
                r#"
                denied=$bin
                RELEASE=0.1.0
                check_foreign
            "#,
            ),
            (
                "status_receipt",
                r#"
                denied=$RECEIPT
                do_status
            "#,
            ),
        ],
    );

    let undo = case(&units, "undo_receipt");
    assert_eq!(undo.code, 0, "{undo:?}");
    assert!(!undo.has_out_line("entered undo journal"), "{undo:?}");
    for name in ["stale_receipt", "stale_entry"] {
        let result = case(&units, name);
        assert_eq!(result.code, 0, "{result:?}");
        assert!(!result.out.contains("watane.new."), "{result:?}");
    }
    for name in ["foreign_receipt", "foreign_entry"] {
        let result = case(&units, name);
        assert_eq!(result.code, 8, "{result:?}");
        assert!(result.has_err_line("  ~/.local/bin/watane"), "{result:?}");
    }
    let status = case(&units, "status_receipt");
    assert_eq!(status.code, 7, "{status:?}");
    assert!(!status.out.contains("journal."), "{status:?}");
}

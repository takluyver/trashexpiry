**Trashexpiry** deletes old items from your (Linux) desktop trash.

Usage:

```shell
# Install (cargo is Rust's package manager - see https://www.rust-lang.org/)
cargo install trashexpiry

# Run now
trashexpiry

# Set up to run daily
trashexpiry --install-timer
```

By default, it deletes files which have been in trash for over 60 days.
This time limit is configurable in `~/.config/trashexpiry.ini`:

```ini
warn_after_days = 50
delete_after_days = 60
```

I wrote this partly to get more familiar with Rust. Use at your own risk.

### Why?

Desktop trash systems normally delete files when you manually empty the
trash. But people often fall into one of two patterns:

* Some delete everything to trash. No disk space is freed, and data you wanted
  to get rid of is still there. When you finally look at the trash, there are
  2000 files there, far too many to think about. You empty it and hope
  there was nothing important there.
* Others either empty trash obsessively, or use shift-delete to bypass it,
  so that you don't need to empty it. This is what I do, and I've found myself
  hard-deleting files only to realise the mistake seconds later.

In contrast, web applications such as GMail have time-limited trash:
you have a few weeks to get things back, and then they're gone for good.
Time-limited trash doesn't pile up, and because I know it will be deleted
automatically, I don't feel a need to keep it clear myself.

So Trashexpiry makes desktop trash behave more like GMail trash.

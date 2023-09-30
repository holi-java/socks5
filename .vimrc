map <S-F10> :wa \| bel term bash -c "RUST_BACKTRACE=0 cargo test --lib -- --nocapture"<CR>
map <C-S-F10> :wa \| bel term bash -c "RUST_BACKTRACE=0 cargo test --tests -- --nocapture"<CR>

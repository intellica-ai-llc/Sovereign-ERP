-- Truth Plane ledger: journal-level conservation lemmas (Round 5; checked with Lean 4.15.0, core library only).

structure Entry where
  account : Nat
  amount  : Int          -- positive = credit, negative = debit
deriving Repr

/-- Net amount of a journal. -/
def total : List Entry → Int
  | []      => 0
  | e :: es => e.amount + total es

def Balanced (es : List Entry) : Prop := total es = 0

/-- A double-entry transfer of `amt` from debit account `d` to credit account `c`. -/
def transfer (d c amt : Nat) : List Entry :=
  [⟨d, -(amt : Int)⟩, ⟨c, (amt : Int)⟩]

/-- Reversal (used by void and by corrections): every entry negated. -/
def reverse : List Entry → List Entry
  | []      => []
  | e :: es => ⟨e.account, -e.amount⟩ :: reverse es

theorem total_append (a b : List Entry) : total (a ++ b) = total a + total b := by
  induction a with
  | nil => simp [total]
  | cons e t ih => simp [total, ih]; omega

theorem transfer_balanced (d c amt : Nat) : Balanced (transfer d c amt) := by
  simp [Balanced, transfer, total]
  omega

theorem balanced_append {a b : List Entry} (ha : Balanced a) (hb : Balanced b) :
    Balanced (a ++ b) := by
  unfold Balanced at *
  rw [total_append, ha, hb]; rfl

theorem total_reverse (es : List Entry) : total (reverse es) = - total es := by
  induction es with
  | nil => rfl
  | cons e t ih => simp [reverse, total, ih]; omega

/-- Voiding a pending journal nets to zero, whether or not the journal was balanced. -/
theorem void_nets_to_zero (es : List Entry) : total (es ++ reverse es) = 0 := by
  rw [total_append, total_reverse]; omega

/-- A journal made only of transfers. -/
def journalOf : List (Nat × Nat × Nat) → List Entry
  | []                => []
  | (d, c, a) :: rest => transfer d c a ++ journalOf rest

/-- Ledger-kernel invariant I1 at journal level: any sequence of transfers conserves value. -/
theorem journalOf_balanced (ts : List (Nat × Nat × Nat)) : Balanced (journalOf ts) := by
  induction ts with
  | nil => rfl
  | cons t rest ih =>
    obtain ⟨d, c, a⟩ := t
    exact balanced_append (transfer_balanced d c a) ih

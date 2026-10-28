-- Closed-period lemmas for the Truth Plane (Round 7; checked with Lean 4.15.0, core library only).

structure PEntry where
  account : Nat
  period  : Nat
  amount  : Int
deriving Repr

/-- Net amount of the entries that fall in period `p`. -/
def periodTotal (p : Nat) : List PEntry → Int
  | []      => 0
  | e :: es => (if e.period = p then e.amount else 0) + periodTotal p es

theorem periodTotal_append (p : Nat) (a b : List PEntry) :
    periodTotal p (a ++ b) = periodTotal p a + periodTotal p b := by
  induction a with
  | nil => simp [periodTotal]
  | cons e t ih => simp [periodTotal, ih]; omega

theorem periodTotal_other (p : Nat) (new : List PEntry) (h : ∀ e ∈ new, e.period ≠ p) :
    periodTotal p new = 0 := by
  induction new with
  | nil => rfl
  | cons e t ih =>
    have he : e.period ≠ p := h e (List.mem_cons_self e t)
    have ht : ∀ x ∈ t, x.period ≠ p := fun x hx => h x (List.mem_cons_of_mem e hx)
    simp [periodTotal, he, ih ht]

/-- Appending entries that belong to other periods never changes a closed period's total. -/
theorem closed_period_unchanged (p : Nat) (es new : List PEntry)
    (h : ∀ e ∈ new, e.period ≠ p) :
    periodTotal p (es ++ new) = periodTotal p es := by
  rw [periodTotal_append, periodTotal_other p new h]; omega

/-- An adjustment to a closed period is posted into a later period with a back-reference; it
    leaves the closed period's total unchanged. -/
theorem adjustment_leaves_closed_total (p q : Nat) (hq : q ≠ p) (es : List PEntry)
    (d c : Nat) (amt : Int) :
    periodTotal p (es ++ [⟨d, q, -amt⟩, ⟨c, q, amt⟩]) = periodTotal p es := by
  apply closed_period_unchanged
  intro e he
  simp at he
  rcases he with rfl | rfl <;> simpa using hq

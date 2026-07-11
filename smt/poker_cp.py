#!/usr/bin/env python3
"""
Poker Deck Optimization using Google OR-Tools CP-SAT Solver

Finds the deck ordering that maximizes the number of cut positions at which
player 0 (the dealer) wins, using constraint programming.

Framed as a Maximize objective rather than a single feasibility check: even
under a timeout, CP-SAT reports the best deck found so far plus a proven
upper bound on how good any deck could possibly be. If the optimal is proven
equal to num_cuts, a perfect deck exists (and this is it). If the optimal is
proven less than num_cuts, no perfect deck exists -- and you still get the
best achievable deck instead of a bare "infeasible".
"""

import argparse
import time
from ortools.sat.python import cp_model


class PokerCPSolver:
    def __init__(self, num_players=2, num_cuts=52):
        self.num_players = num_players
        self.num_cuts = num_cuts
        self.model = cp_model.CpModel()
        self.deck = []

        # Card encoding: card_id = suit * 13 + value
        # suit: 0=clubs, 1=diamonds, 2=hearts, 3=spades
        # value: 0=2, 1=3, ..., 11=K, 12=A

    def card_to_string(self, card_id):
        """Convert card ID to readable string like '2c', 'Ah'"""
        suit = card_id // 13
        value = card_id % 13

        suit_char = ['c', 'd', 'h', 's'][suit]
        value_str = ['2', '3', '4', '5', '6', '7', '8', '9', '10', 'J', 'Q', 'K', 'A'][value]

        return f"{value_str}{suit_char}"

    def create_deck_variables(self):
        """Create 52 integer variables for deck positions"""
        print("Creating deck variables (52 cards)...")
        self.deck = [self.model.NewIntVar(0, 51, f'card_{i}') for i in range(52)]

        # All cards must be distinct (valid permutation)
        print("Adding permutation constraint...")
        self.model.AddAllDifferent(self.deck)

    def apply_hint(self, hint_deck):
        """Seed the search with a known-good deck (e.g. from the heuristic
        search or a previous solve), via CP-SAT's solution hint mechanism."""
        assert len(hint_deck) == 52, "hint deck must have exactly 52 cards"
        for card_var, val in zip(self.deck, hint_deck):
            self.model.AddHint(card_var, val)

    # -- reification helpers -------------------------------------------------
    # CP-SAT booleans built from `.OnlyEnforceIf(x)` alone only constrain the
    # forward direction (x==True implies the condition holds); nothing stops
    # the solver from setting x=False even when the condition is genuinely
    # true unless the reverse direction is *also* asserted. Every boolean
    # built below is fully (iff) reified, both directions, so the solver can
    # never just "opt out" of reporting a flush/straight/pair it doesn't like.

    def reify_eq(self, a, b):
        """BoolVar r with r <=> (a == b)."""
        r = self.model.NewBoolVar('')
        self.model.Add(a == b).OnlyEnforceIf(r)
        self.model.Add(a != b).OnlyEnforceIf(r.Not())
        return r

    def reify_and(self, bools):
        """BoolVar r with r <=> AND(bools)."""
        r = self.model.NewBoolVar('')
        self.model.AddBoolAnd(bools).OnlyEnforceIf(r)
        self.model.AddBoolOr([b.Not() for b in bools]).OnlyEnforceIf(r.Not())
        return r

    def reify_or(self, bools):
        """BoolVar r with r <=> OR(bools)."""
        r = self.model.NewBoolVar('')
        self.model.AddBoolOr(bools).OnlyEnforceIf(r)
        self.model.AddBoolAnd([b.Not() for b in bools]).OnlyEnforceIf(r.Not())
        return r

    def extract_card_properties(self, card_var):
        """
        Extract suit and value from a card variable.
        suit = card // 13, value = card % 13. This is a plain equality
        (suit*13 + value == card, with suit in 0..3 and value in 0..12), so
        it's a complete, sound definition -- no reification needed.
        """
        suit = self.model.NewIntVar(0, 3, '')
        value = self.model.NewIntVar(0, 12, '')
        self.model.Add(card_var == suit * 13 + value)
        return suit, value

    def sort5(self, values):
        """
        Sort 5 IntVars ascending with a fixed 9-comparator sorting network
        (optimal for n=5), using AddMinEquality/AddMaxEquality -- native,
        fully-constrained global constraints -- for each compare-exchange.
        Everything downstream (straight detection, pair pattern, high card)
        reads off this sorted order instead of a per-value existential
        search.
        """
        v = list(values)
        network = [(0, 1), (3, 4), (2, 4), (2, 3), (0, 3), (0, 2), (1, 4), (1, 3), (1, 2)]
        for (i, j) in network:
            lo = self.model.NewIntVar(0, 12, '')
            hi = self.model.NewIntVar(0, 12, '')
            self.model.AddMinEquality(lo, [v[i], v[j]])
            self.model.AddMaxEquality(hi, [v[i], v[j]])
            v[i], v[j] = lo, hi
        return v

    def hand_score(self, cards):
        """
        Score a 5-card hand as a single comparable integer: rank * 100 + hi,
        matching the simplified (rank, high_card) scoring the rest of this
        project uses (see ../src/hands.rs::score_five_cards) so a solution
        found here means the same thing as a win anywhere else in the
        codebase.

        Once the 5 values are sorted, equal values are necessarily
        contiguous, so the whole pair/two-pair/trips/full-house/quad pattern
        is determined by just the 4 adjacent-equality booleans. Straight and
        flush can only occur when all 5 values are distinct (a repeated
        value rules out 5 consecutive values, and a repeated suit would
        require a repeated (suit, value) card, which AllDifferent on the
        deck rules out), so the pair-pattern ranks and the straight/flush
        ranks never collide -- both families can be summed directly as
        mutually-exclusive weighted booleans, no priority cascade needed.
        """
        values = [self.extract_card_properties(c)[1] for c in cards]
        suits = [self.extract_card_properties(c)[0] for c in cards]
        v = self.sort5(values)

        e01 = self.reify_eq(v[0], v[1])
        e12 = self.reify_eq(v[1], v[2])
        e23 = self.reify_eq(v[2], v[3])
        e34 = self.reify_eq(v[3], v[4])
        n01, n12, n23, n34 = e01.Not(), e12.Not(), e23.Not(), e34.Not()

        is_flush = self.reify_and([
            self.reify_eq(suits[0], suits[1]),
            self.reify_eq(suits[0], suits[2]),
            self.reify_eq(suits[0], suits[3]),
            self.reify_eq(suits[0], suits[4]),
        ])

        quad = self.reify_or([
            self.reify_and([e12, e23, e34, n01]),
            self.reify_and([e01, e12, e23, n34]),
        ])
        full_house = self.reify_or([
            self.reify_and([e01, e23, e34, n12]),
            self.reify_and([e01, e12, e34, n23]),
        ])
        trips = self.reify_or([
            self.reify_and([e01, e12, n23, n34]),
            self.reify_and([e12, e23, n01, n34]),
            self.reify_and([e23, e34, n01, n12]),
        ])
        two_pair = self.reify_or([
            self.reify_and([e01, e23, n12, n34]),
            self.reify_and([e01, e34, n12, n23]),
            self.reify_and([e12, e34, n01, n23]),
        ])
        one_pair = self.reify_or([
            self.reify_and([e01, n12, n23, n34]),
            self.reify_and([e12, n01, n23, n34]),
            self.reify_and([e23, n01, n12, n34]),
            self.reify_and([e34, n01, n12, n23]),
        ])

        is_wheel = self.reify_and([
            self.reify_eq(v[0], 0),
            self.reify_eq(v[1], 1),
            self.reify_eq(v[2], 2),
            self.reify_eq(v[3], 3),
            self.reify_eq(v[4], 12),
        ])
        consecutive = self.reify_and([
            self.reify_eq(v[1], v[0] + 1),
            self.reify_eq(v[2], v[1] + 1),
            self.reify_eq(v[3], v[2] + 1),
            self.reify_eq(v[4], v[3] + 1),
        ])
        is_straight = self.reify_or([is_wheel, consecutive])

        straight_only = self.reify_and([is_straight, is_flush.Not()])
        flush_only = self.reify_and([is_flush, is_straight.Not()])
        straight_flush = self.reify_and([is_straight, is_flush])

        rank = self.model.NewIntVar(0, 8, '')
        self.model.Add(
            rank == 1 * one_pair + 2 * two_pair + 3 * trips + 6 * full_house + 7 * quad
            + 4 * straight_only + 5 * flush_only + 8 * straight_flush
        )

        # High card tiebreaker, matching hands.rs::score_five_cards exactly:
        # an ace counts as 14 unless it's completing the wheel, where the
        # "5" (v[3]) is the effective high card. These three cases are
        # mutually exclusive and exhaustive, so pinning hi with three
        # OnlyEnforceIf branches is sound (exactly one always fires).
        has_ace = self.reify_eq(v[4], 12)
        ace_not_wheel = self.reify_and([has_ace, is_wheel.Not()])
        not_ace = has_ace.Not()

        hi = self.model.NewIntVar(2, 14, '')
        self.model.Add(hi == 5).OnlyEnforceIf(is_wheel)
        self.model.Add(hi == 14).OnlyEnforceIf(ace_not_wheel)
        self.model.Add(hi == v[4] + 2).OnlyEnforceIf(not_ace)

        score = self.model.NewIntVar(2, 814, '')
        self.model.Add(score == rank * 100 + hi)
        return score

    def best_hand_from_seven(self, hole_cards, community):
        """
        Best hand score from 7 cards (2 hole + 5 community): the max
        hand_score over all C(7,5) = 21 combinations. AddMaxEquality is a
        native, fully-reified global constraint, so this also fixes the old
        code's bug of grabbing tiebreakers from an arbitrary combination --
        there's only a single combined score now, and the max is exact.
        """
        all_cards = hole_cards + community
        combinations = [
            [0, 1, 2, 3, 4], [0, 1, 2, 3, 5], [0, 1, 2, 3, 6], [0, 1, 2, 4, 5], [0, 1, 2, 4, 6],
            [0, 1, 2, 5, 6], [0, 1, 3, 4, 5], [0, 1, 3, 4, 6], [0, 1, 3, 5, 6], [0, 1, 4, 5, 6],
            [0, 2, 3, 4, 5], [0, 2, 3, 4, 6], [0, 2, 3, 5, 6], [0, 2, 4, 5, 6], [0, 3, 4, 5, 6],
            [1, 2, 3, 4, 5], [1, 2, 3, 4, 6], [1, 2, 3, 5, 6], [1, 2, 4, 5, 6], [1, 3, 4, 5, 6],
            [2, 3, 4, 5, 6],
        ]

        scores = [self.hand_score([all_cards[i] for i in combo]) for combo in combinations]
        best = self.model.NewIntVar(2, 814, '')
        self.model.AddMaxEquality(best, scores)
        return best

    def add_game_constraints_and_objective(self):
        """
        For each cut position, add a boolean "dealer wins this cut"
        indicator (fully reified), then maximize the sum across all cuts
        instead of asserting every one must hold. This makes the solve
        anytime: even on a timeout you get the best deck found so far and a
        proven upper bound, rather than a bare UNKNOWN.
        """
        print(f"Generating constraints for all {self.num_cuts} cut positions...")
        print("(This may take a few minutes)")
        print()

        win_indicators = []
        for cut in range(self.num_cuts):
            if cut % 10 == 0:
                print(f"  Processing cut position {cut}/{self.num_cuts}...")

            cut_deck = [self.deck[(cut + i) % 52] for i in range(52)]

            player_hands = [
                [cut_deck[2 * p], cut_deck[2 * p + 1]] for p in range(self.num_players)
            ]
            community = [cut_deck[2 * self.num_players + i] for i in range(5)]

            player_scores = [
                self.best_hand_from_seven(player_hands[p], community)
                for p in range(self.num_players)
            ]

            beats_all = []
            for p in range(1, self.num_players):
                b = self.model.NewBoolVar('')
                self.model.Add(player_scores[0] > player_scores[p]).OnlyEnforceIf(b)
                self.model.Add(player_scores[0] <= player_scores[p]).OnlyEnforceIf(b.Not())
                beats_all.append(b)

            win = self.reify_and(beats_all)
            win_indicators.append(win)

        self.win_indicators = win_indicators
        self.model.Maximize(sum(win_indicators))

        print()
        print("All constraints generated!")

    def solve(self, time_limit_seconds=3600):
        """Solve the maximization model."""
        print(f"Starting CP-SAT solver (timeout: {time_limit_seconds}s)...")
        print()

        solver = cp_model.CpSolver()
        solver.parameters.max_time_in_seconds = time_limit_seconds
        solver.parameters.log_search_progress = True
        solver.parameters.num_search_workers = 8  # Parallel search

        start_time = time.time()
        status = solver.Solve(self.model)
        elapsed = time.time() - start_time

        print()
        print(f"Solver finished in {elapsed:.2f} seconds (status: {solver.StatusName(status)})")
        print()

        if status not in (cp_model.OPTIMAL, cp_model.FEASIBLE):
            bound = int(solver.BestObjectiveBound())
            print(f"No feasible deck found within the time limit. Best known upper bound: {bound}/{self.num_cuts}.")
            return None

        best = int(solver.ObjectiveValue())
        bound = int(solver.BestObjectiveBound())

        if status == cp_model.OPTIMAL:
            if best == self.num_cuts:
                print(f"✓ PROVEN OPTIMAL: a perfect deck exists -- dealer wins all {self.num_cuts}/{self.num_cuts} cuts.")
            else:
                print(f"✓ PROVEN OPTIMAL: the best any deck can do is {best}/{self.num_cuts} -- "
                      f"a perfect deck ({self.num_cuts}/{self.num_cuts}) is impossible.")
        else:
            print(f"Best found so far: {best}/{self.num_cuts} wins. "
                  f"Proven upper bound: {bound}/{self.num_cuts} (hit the time limit, not proven optimal).")

        solution = [solver.Value(c) for c in self.deck]
        print()
        print("Best deck ordering found:")
        for i, card_val in enumerate(solution):
            print(f"  Position {i:2d}: {self.card_to_string(card_val)}")

        print()
        print("Deck as comma-separated card IDs:")
        print(','.join(map(str, solution)))

        return solution, best, bound


def main():
    parser = argparse.ArgumentParser(
        description='Poker deck optimization using OR-Tools CP-SAT'
    )
    parser.add_argument(
        '-n', '--num-players',
        type=int,
        default=2,
        help='Number of players including dealer (default: 2)'
    )
    parser.add_argument(
        '-t', '--timeout',
        type=int,
        default=3600,
        help='Solver timeout in seconds (default: 3600)'
    )
    parser.add_argument(
        '-c', '--num-cuts',
        type=int,
        default=52,
        help='Number of cut positions to check (default: 52)'
    )
    parser.add_argument(
        '--hint',
        type=str,
        default=None,
        help='Path to a file containing a comma-separated 52-card-id deck to seed the search with'
    )

    args = parser.parse_args()

    print("=" * 60)
    print("Poker Deck Optimization - OR-Tools CP-SAT Solver")
    print("=" * 60)
    print(f"Players: {args.num_players}")
    print(f"Cut positions: {args.num_cuts}")
    print(f"Timeout: {args.timeout} seconds")
    print()

    solver = PokerCPSolver(num_players=args.num_players, num_cuts=args.num_cuts)
    solver.create_deck_variables()
    if args.hint:
        with open(args.hint) as f:
            hint_deck = [int(x) for x in f.read().strip().split(',')]
        solver.apply_hint(hint_deck)
    solver.add_game_constraints_and_objective()
    result = solver.solve(time_limit_seconds=args.timeout)

    if result:
        print()
        print("✓ Success!")


if __name__ == '__main__':
    main()

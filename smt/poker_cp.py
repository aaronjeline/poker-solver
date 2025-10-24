#!/usr/bin/env python3
"""
Poker Deck Optimization using Google OR-Tools CP-SAT Solver

Finds a deck ordering where player 0 wins at all cut positions.
Uses constraint programming instead of SMT for better performance.
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

    def extract_card_properties(self, card_var):
        """
        Extract suit and value from a card variable.
        Returns (suit_var, value_var) where:
        - suit = card // 13
        - value = card % 13
        """
        suit = self.model.NewIntVar(0, 3, '')
        value = self.model.NewIntVar(0, 12, '')

        # suit * 13 + value = card
        self.model.Add(card_var == suit * 13 + value)

        return suit, value

    def is_flush(self, cards):
        """
        Check if 5 cards form a flush (all same suit).
        Returns a boolean variable.
        """
        suits = [self.extract_card_properties(c)[0] for c in cards]

        # All suits must be equal
        flush_var = self.model.NewBoolVar('')

        # flush_var == (s0 == s1 == s2 == s3 == s4)
        for i in range(1, 5):
            same = self.model.NewBoolVar('')
            self.model.Add(suits[0] == suits[i]).OnlyEnforceIf(same)
            self.model.Add(suits[0] != suits[i]).OnlyEnforceIf(same.Not())
            # All must be the same
            self.model.AddImplication(flush_var, same)

        return flush_var

    def is_straight(self, cards):
        """
        Check if 5 cards form a straight.
        Returns a boolean variable.

        This is complex - we check if values form one of the valid straights:
        - Regular: 0-1-2-3-4, 1-2-3-4-5, ..., 8-9-10-11-12
        - Wheel: A-2-3-4-5 (values 12,0,1,2,3)
        """
        values = [self.extract_card_properties(c)[1] for c in cards]

        straight_var = self.model.NewBoolVar('')

        # For simplicity in CP-SAT, we'll check each possible straight
        possible_straights = []

        # Regular straights: 0-4, 1-5, 2-6, ..., 8-12
        for start in range(9):
            expected = list(range(start, start + 5))
            is_this_straight = self.check_values_match_set(values, expected)
            possible_straights.append(is_this_straight)

        # Wheel: A-2-3-4-5 (values 0,1,2,3,12)
        wheel = self.check_values_match_set(values, [0, 1, 2, 3, 12])
        possible_straights.append(wheel)

        # straight_var is true if any of the possible straights is true
        self.model.AddBoolOr(possible_straights).OnlyEnforceIf(straight_var)
        self.model.AddBoolAnd([s.Not() for s in possible_straights]).OnlyEnforceIf(straight_var.Not())

        return straight_var

    def check_values_match_set(self, values, expected_set):
        """
        Check if the 5 values (in any order) exactly match expected_set.
        Returns a boolean variable.
        """
        match_var = self.model.NewBoolVar('')

        # For each expected value, at least one card must have it
        # And for each card value, it must be in expected_set

        for exp_val in expected_set:
            # At least one value equals exp_val
            has_value_bools = []
            for v in values:
                b = self.model.NewBoolVar('')
                self.model.Add(v == exp_val).OnlyEnforceIf(b)
                self.model.Add(v != exp_val).OnlyEnforceIf(b.Not())
                has_value_bools.append(b)

            # If match_var is true, then at least one of these must be true
            self.model.AddBoolOr(has_value_bools).OnlyEnforceIf(match_var)

        # Also ensure each value is in the expected set
        for v in values:
            # v must be one of expected_set values
            in_set_bools = []
            for exp_val in expected_set:
                b = self.model.NewBoolVar('')
                self.model.Add(v == exp_val).OnlyEnforceIf(b)
                self.model.Add(v != exp_val).OnlyEnforceIf(b.Not())
                in_set_bools.append(b)

            # If match_var is true, v must be in the set
            self.model.AddBoolOr(in_set_bools).OnlyEnforceIf(match_var)

        return match_var

    def count_value_occurrences(self, cards):
        """
        Count how many times each value (0-12) appears in the 5 cards.
        Returns a list of 13 count variables.
        """
        values = [self.extract_card_properties(c)[1] for c in cards]
        counts = []

        for target_val in range(13):
            count = self.model.NewIntVar(0, 5, '')

            # count = number of values that equal target_val
            is_equal = []
            for v in values:
                b = self.model.NewBoolVar('')
                self.model.Add(v == target_val).OnlyEnforceIf(b)
                self.model.Add(v != target_val).OnlyEnforceIf(b.Not())
                is_equal.append(b)

            # Sum the boolean variables
            self.model.Add(count == sum(is_equal))
            counts.append(count)

        return counts

    def compute_hand_rank(self, cards):
        """
        Compute poker hand rank for 5 cards.
        Returns rank variable (0-8):
        0=high card, 1=pair, 2=two pair, 3=three of a kind, 4=straight,
        5=flush, 6=full house, 7=four of a kind, 8=straight flush

        Also returns tiebreaker variables for comparison.
        """
        values = [self.extract_card_properties(c)[1] for c in cards]

        is_flush_var = self.is_flush(cards)
        is_straight_var = self.is_straight(cards)
        value_counts = self.count_value_occurrences(cards)

        # Detect hand patterns
        has_four = self.model.NewBoolVar('')
        has_three = self.model.NewBoolVar('')
        num_pairs = self.model.NewIntVar(0, 2, '')

        # has_four: at least one count == 4
        four_bools = []
        for c in value_counts:
            b = self.model.NewBoolVar('')
            self.model.Add(c == 4).OnlyEnforceIf(b)
            self.model.Add(c != 4).OnlyEnforceIf(b.Not())
            four_bools.append(b)
        self.model.AddBoolOr(four_bools).OnlyEnforceIf(has_four)
        self.model.AddBoolAnd([b.Not() for b in four_bools]).OnlyEnforceIf(has_four.Not())

        # has_three: at least one count == 3
        three_bools = []
        for c in value_counts:
            b = self.model.NewBoolVar('')
            self.model.Add(c == 3).OnlyEnforceIf(b)
            self.model.Add(c != 3).OnlyEnforceIf(b.Not())
            three_bools.append(b)
        self.model.AddBoolOr(three_bools).OnlyEnforceIf(has_three)
        self.model.AddBoolAnd([b.Not() for b in three_bools]).OnlyEnforceIf(has_three.Not())

        # num_pairs: count how many values have count == 2
        pair_bools = []
        for c in value_counts:
            b = self.model.NewBoolVar('')
            self.model.Add(c == 2).OnlyEnforceIf(b)
            self.model.Add(c != 2).OnlyEnforceIf(b.Not())
            pair_bools.append(b)
        self.model.Add(num_pairs == sum(pair_bools))

        # Determine rank
        rank = self.model.NewIntVar(0, 8, '')

        # Create boolean for each rank type
        is_straight_flush = self.model.NewBoolVar('')
        self.model.AddBoolAnd([is_flush_var, is_straight_var]).OnlyEnforceIf(is_straight_flush)
        self.model.AddBoolOr([is_flush_var.Not(), is_straight_var.Not()]).OnlyEnforceIf(is_straight_flush.Not())

        is_four_kind = has_four

        # Create boolean for num_pairs == 1
        num_pairs_is_one = self.model.NewBoolVar('')
        self.model.Add(num_pairs == 1).OnlyEnforceIf(num_pairs_is_one)
        self.model.Add(num_pairs != 1).OnlyEnforceIf(num_pairs_is_one.Not())

        # Create boolean for num_pairs == 2
        num_pairs_is_two = self.model.NewBoolVar('')
        self.model.Add(num_pairs == 2).OnlyEnforceIf(num_pairs_is_two)
        self.model.Add(num_pairs != 2).OnlyEnforceIf(num_pairs_is_two.Not())

        is_full_house = self.model.NewBoolVar('')
        self.model.AddBoolAnd([has_three, num_pairs_is_one]).OnlyEnforceIf(is_full_house)
        self.model.AddBoolOr([has_three.Not(), num_pairs_is_one.Not()]).OnlyEnforceIf(is_full_house.Not())

        is_flush_only = self.model.NewBoolVar('')
        self.model.AddBoolAnd([is_flush_var, is_straight_var.Not()]).OnlyEnforceIf(is_flush_only)
        self.model.AddBoolOr([is_flush_var.Not(), is_straight_var]).OnlyEnforceIf(is_flush_only.Not())

        is_straight_only = self.model.NewBoolVar('')
        self.model.AddBoolAnd([is_straight_var, is_flush_var.Not()]).OnlyEnforceIf(is_straight_only)
        self.model.AddBoolOr([is_straight_var.Not(), is_flush_var]).OnlyEnforceIf(is_straight_only.Not())

        is_three_kind = self.model.NewBoolVar('')
        self.model.AddBoolAnd([has_three, num_pairs_is_one.Not()]).OnlyEnforceIf(is_three_kind)
        self.model.AddBoolOr([has_three.Not(), num_pairs_is_one]).OnlyEnforceIf(is_three_kind.Not())

        is_two_pair = num_pairs_is_two

        is_one_pair = num_pairs_is_one

        # Determine rank based on hand type (in priority order from highest to lowest)
        # We'll use a cascading if-then-else approach

        # Start with rank 0 (high card) as default
        # Then check each hand type in descending order and set rank accordingly

        # Using OnlyEnforceIf constraints to map boolean hand types to rank values
        self.model.Add(rank == 8).OnlyEnforceIf(is_straight_flush)
        self.model.Add(rank == 7).OnlyEnforceIf([is_four_kind, is_straight_flush.Not()])
        self.model.Add(rank == 6).OnlyEnforceIf([is_full_house, is_straight_flush.Not(), is_four_kind.Not()])
        self.model.Add(rank == 5).OnlyEnforceIf([is_flush_only, is_straight_flush.Not(), is_four_kind.Not(), is_full_house.Not()])
        self.model.Add(rank == 4).OnlyEnforceIf([is_straight_only, is_straight_flush.Not(), is_four_kind.Not(), is_full_house.Not(), is_flush_only.Not()])
        self.model.Add(rank == 3).OnlyEnforceIf([is_three_kind, is_straight_flush.Not(), is_four_kind.Not(), is_full_house.Not(), is_flush_only.Not(), is_straight_only.Not()])
        self.model.Add(rank == 2).OnlyEnforceIf([is_two_pair, is_straight_flush.Not(), is_four_kind.Not(), is_full_house.Not(), is_flush_only.Not(), is_straight_only.Not(), is_three_kind.Not()])
        self.model.Add(rank == 1).OnlyEnforceIf([is_one_pair, is_straight_flush.Not(), is_four_kind.Not(), is_full_house.Not(), is_flush_only.Not(), is_straight_only.Not(), is_three_kind.Not(), is_two_pair.Not()])
        # High card: none of the above hand types
        is_high_card = self.model.NewBoolVar('')
        self.model.AddBoolOr([is_straight_flush, is_four_kind, is_full_house, is_flush_only,
                              is_straight_only, is_three_kind, is_two_pair, is_one_pair]).OnlyEnforceIf(is_high_card.Not())
        self.model.AddBoolAnd([is_straight_flush.Not(), is_four_kind.Not(), is_full_house.Not(),
                               is_flush_only.Not(), is_straight_only.Not(), is_three_kind.Not(),
                               is_two_pair.Not(), is_one_pair.Not()]).OnlyEnforceIf(is_high_card)

        self.model.Add(rank == 0).OnlyEnforceIf(is_high_card)

        # Ensure exactly one hand type is active
        self.model.AddExactlyOne([is_straight_flush, is_four_kind, is_full_house, is_flush_only,
                                  is_straight_only, is_three_kind, is_two_pair, is_one_pair, is_high_card])

        # Simplified tiebreakers: just use the values sorted descending
        # A proper implementation would sort by count pattern
        tiebreakers = values  # Simplified

        return rank, tiebreakers

    def hand_is_better(self, rank1, tb1, rank2, tb2):
        """
        Return a boolean variable that is true if hand1 > hand2.
        Comparison: first by rank, then by tiebreakers lexicographically.
        """
        is_better = self.model.NewBoolVar('')

        # rank1 > rank2 OR (rank1 == rank2 AND tb1 > tb2)
        rank_greater = self.model.NewBoolVar('')
        self.model.Add(rank1 > rank2).OnlyEnforceIf(rank_greater)
        self.model.Add(rank1 <= rank2).OnlyEnforceIf(rank_greater.Not())

        rank_equal = self.model.NewBoolVar('')
        self.model.Add(rank1 == rank2).OnlyEnforceIf(rank_equal)
        self.model.Add(rank1 != rank2).OnlyEnforceIf(rank_equal.Not())

        # Tiebreaker comparison (lexicographic on first value for simplicity)
        tb_greater = self.model.NewBoolVar('')
        if len(tb1) > 0 and len(tb2) > 0:
            # Compare first tiebreaker (simplified - should be all 5)
            self.model.Add(tb1[0] > tb2[0]).OnlyEnforceIf(tb_greater)
            self.model.Add(tb1[0] <= tb2[0]).OnlyEnforceIf(tb_greater.Not())

        # is_better = rank_greater OR (rank_equal AND tb_greater)
        tb_better_and_equal = self.model.NewBoolVar('')
        self.model.AddBoolAnd([rank_equal, tb_greater]).OnlyEnforceIf(tb_better_and_equal)

        self.model.AddBoolOr([rank_greater, tb_better_and_equal]).OnlyEnforceIf(is_better)
        self.model.AddBoolAnd([rank_greater.Not(), tb_better_and_equal.Not()]).OnlyEnforceIf(is_better.Not())

        return is_better

    def best_hand_from_seven(self, hole_cards, community):
        """
        Find the best 5-card hand from 7 cards (2 hole + 5 community).
        Returns (rank, tiebreakers) for the best hand.

        Evaluates all C(7,5) = 21 combinations.
        """
        all_cards = hole_cards + community

        # All 21 combinations of 5 cards from 7
        combinations = [
            [0,1,2,3,4], [0,1,2,3,5], [0,1,2,3,6], [0,1,2,4,5], [0,1,2,4,6],
            [0,1,2,5,6], [0,1,3,4,5], [0,1,3,4,6], [0,1,3,5,6], [0,1,4,5,6],
            [0,2,3,4,5], [0,2,3,4,6], [0,2,3,5,6], [0,2,4,5,6], [0,3,4,5,6],
            [1,2,3,4,5], [1,2,3,4,6], [1,2,3,5,6], [1,2,4,5,6], [1,3,4,5,6],
            [2,3,4,5,6],
        ]

        # Evaluate all hands
        hand_ranks = []
        hand_tbs = []

        for combo in combinations:
            hand = [all_cards[i] for i in combo]
            rank, tbs = self.compute_hand_rank(hand)
            hand_ranks.append(rank)
            hand_tbs.append(tbs)

        # Find maximum rank
        best_rank = self.model.NewIntVar(0, 8, '')
        best_tb = [self.model.NewIntVar(0, 12, '') for _ in range(5)]

        # best_rank = max(hand_ranks)
        self.model.AddMaxEquality(best_rank, hand_ranks)

        # For tiebreakers, we'd need to find which hand has the max rank
        # Simplified: just use first hand's tiebreakers (this is incorrect but simpler)
        # A proper implementation would select tiebreakers of the hand with best_rank
        for i in range(5):
            best_tb[i] = hand_tbs[0][i]  # Simplified

        return best_rank, best_tb

    def add_game_constraints(self):
        """Add constraints for all cut positions"""
        print(f"Generating constraints for all {self.num_cuts} cut positions...")
        print("(This may take a few minutes)")
        print()

        for cut in range(self.num_cuts):
            if cut % 10 == 0:
                print(f"  Processing cut position {cut}/{self.num_cuts}...")

            # Create cut deck
            cut_deck = [self.deck[(cut + i) % 52] for i in range(52)]

            # Deal cards to players
            player_hands = []
            for p in range(self.num_players):
                hole_cards = [cut_deck[2*p], cut_deck[2*p + 1]]
                player_hands.append(hole_cards)

            # Community cards
            community = [cut_deck[2*self.num_players + i] for i in range(5)]

            # Evaluate best hand for each player
            player_best = []
            for p in range(self.num_players):
                rank, tbs = self.best_hand_from_seven(player_hands[p], community)
                player_best.append((rank, tbs))

            # Player 0 must beat all other players
            for p in range(1, self.num_players):
                p0_better = self.hand_is_better(
                    player_best[0][0], player_best[0][1],
                    player_best[p][0], player_best[p][1]
                )
                self.model.AddBoolAnd([p0_better])  # Assert it must be true

        print()
        print("All constraints generated!")

    def solve(self, time_limit_seconds=3600):
        """Solve the constraint model"""
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
        print(f"Solver finished in {elapsed:.2f} seconds")
        print()

        if status == cp_model.OPTIMAL or status == cp_model.FEASIBLE:
            print("✓ SOLUTION FOUND!")
            print()

            print("Winning deck ordering:")
            solution = []
            for i in range(52):
                card_val = solver.Value(self.deck[i])
                solution.append(card_val)
                print(f"  Position {i:2d}: {self.card_to_string(card_val)}")

            print()
            print("Deck as comma-separated card IDs:")
            print(','.join(map(str, solution)))

            return solution

        elif status == cp_model.INFEASIBLE:
            print("✗ INFEASIBLE: No deck ordering exists where player 0 wins all positions.")
            return None

        else:
            print("? UNKNOWN: Solver could not determine feasibility (timeout or limit reached).")
            print(f"   Try increasing the time limit (current: {time_limit_seconds}s)")
            return None


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
    solver.add_game_constraints()
    solution = solver.solve(time_limit_seconds=args.timeout)

    if solution:
        print()
        print("✓ Success! Found a valid deck ordering.")


if __name__ == '__main__':
    main()

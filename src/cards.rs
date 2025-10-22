use proptest::prelude::*;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Card(pub u8);

const MAX_CARD: u8 = 51;

impl Card {
    pub fn new(value: Value, suit: Suit) -> Self {
        let scale = u8::from(suit);
        let value = value.0 - 1;
        Self(value + 13 * scale)
    }

    pub fn valid(&self) -> bool {
        self.0 <= MAX_CARD
    }

    pub fn into_inner(self) -> (Value, Suit) {
        let suit = (self.0 / 13);
        let value = (self.0 % 13) + 1;
        (Value::new(value), suit.into())
    }
}

impl Arbitrary for Card {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        (0..=MAX_CARD).prop_map(Card).boxed()
    }
}

impl std::fmt::Display for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (value, suit) = self.into_inner();
        write!(f, "{}{}", value, suit)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Value(pub u8);

impl Value {
    pub fn new(value: u8) -> Self {
        if value > 0 && value <= 13 {
            Self(value)
        } else {
            panic!("Illegal card value");
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Arbitrary for Value {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        (1u8..=13u8).prop_map(Value).boxed()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suit {
    Clubs,
    Spades,
    Hearts,
    Diamonds,
}

impl From<Suit> for u8 {
    fn from(suit: Suit) -> u8 {
        use Suit::*;
        match suit {
            Clubs => 0,
            Spades => 1,
            Hearts => 2,
            Diamonds => 3,
        }
    }
}

impl From<u8> for Suit {
    fn from(value: u8) -> Self {
        use Suit::*;
        match value {
            0 => Clubs,
            1 => Spades,
            2 => Hearts,
            3 => Diamonds,
            _ => panic!("Invalid suit {value}"),
        }
    }
}

impl Arbitrary for Suit {
    type Parameters = ();
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        prop_oneof![
            Just(Suit::Clubs),
            Just(Suit::Spades),
            Just(Suit::Hearts),
            Just(Suit::Diamonds),
        ]
        .boxed()
    }
}

impl std::fmt::Display for Suit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Suit::Clubs => write!(f, "♣"),
            Suit::Spades => write!(f, "♠"),
            Suit::Hearts => write!(f, "♥"),
            Suit::Diamonds => write!(f, "♦"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example() {
        let c = Card(0);
        let (value, suit) = c.into_inner();
        assert_eq!(value, Value::new(1));
        assert_eq!(suit, Suit::Clubs)
    }

    proptest! {
        #[test]
        fn test_value_always_in_valid_range(value in any::<Value>()) {
            // Value should always be between 1 and 13 inclusive
            let inner = value.0;
            prop_assert!(inner >= 1 && inner <= 13);
        }

        #[test]
        fn test_suit_roundtrip_conversion(suit in any::<Suit>()) {
            // Converting Suit -> u8 -> Suit should return the original
            let as_u8: u8 = suit.clone().into();
            let back_to_suit = Suit::from(as_u8);

            // Compare by converting both to u8 since Suit doesn't derive PartialEq
            let original_u8: u8 = suit.into();
            let roundtrip_u8: u8 = back_to_suit.into();
            prop_assert_eq!(original_u8, roundtrip_u8);
        }

        #[test]
        fn test_suit_to_u8_in_range(suit in any::<Suit>()) {
            // Suit should always convert to 0-3
            let as_u8: u8 = suit.into();
            prop_assert!(as_u8 <= 3);
        }

        #[test]
        fn test_card_has_valid_components(card in any::<Card>()) {
            prop_assert!(card.valid());

        }

        #[test]
        fn test_card_roundtrip_conversion(card in any::<Card>()) {
            // Converting Card -> u8 -> Card should preserve suit and value
            let (value, suit) = card.into_inner();
            let card2 = Card::new(value, suit);
            prop_assert_eq!(card, card2);
        }
    }
}

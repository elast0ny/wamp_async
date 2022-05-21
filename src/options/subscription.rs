use crate::{
    WampDict,
    Arg
};
use crate::options::option::{
    OptionBuilder,
    WampOption,
};

pub struct SubscriptionOptionItem(Option<WampDict>);

impl SubscriptionOptionItem {
    pub fn with_match(&self, match_option: &str) -> Self {
        self.with_option(WampOption::SubscribeOption("match".to_owned(), Arg::String(match_option.to_owned())))
    }
}

impl OptionBuilder for SubscriptionOptionItem {
    fn create(options: Option<WampDict>) -> Self where Self: OptionBuilder + Sized {
        Self(options)
    }

    fn get_dict(&self) -> Option<WampDict> {
        self.0.clone()
    }
}

impl Default for SubscriptionOptionItem {
    fn default() -> Self {
        Self::empty()
    }
}

pub type SubscribeOptions = SubscriptionOptionItem;

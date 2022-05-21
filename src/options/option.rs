use crate::{
    Arg,
    WampDict,
    WampString,
};

#[derive(Debug, Clone)]
pub enum WampOption<K, V> {
    PublishOption(K, V),
    SubscribeOption(K, V),
    CallOption(K, V),
    RegisterOption(K, V),
    None
}

pub trait OptionBuilder {

    fn with_option(&self, option: WampOption<String, Arg>) -> Self where Self: OptionBuilder + Sized {
        let mut next_options = match &self.get_dict() {
            Some(opts) => opts.clone(),
            None => WampDict::new()
        };

        let (key, value) = match Self::validate_option(option.clone()) {
            Some(result) => result,
            None => panic!("Can't create invalid option {:?}", option)
        };

        next_options.insert(key, value);

        Self::create(Some(next_options.clone()))
    }

    // TODO: Actual validation per role here
    fn validate_option(option: WampOption<String, Arg>) -> Option<(WampString, Arg)> {
        match option {
            WampOption::PublishOption(key, value) => Some((key, value)),
            WampOption::SubscribeOption(key, value) => Some((key, value)),
            WampOption::RegisterOption(key, value) => Some((key, value)),
            WampOption::CallOption(key, value) => Some((key, value)),
            WampOption::None => None,
        }
    }
    
    fn new() -> Self where Self: OptionBuilder + Sized {
        Self::empty()
    }

    fn empty() -> Self where Self: OptionBuilder + Sized {
        Self::create(None)
    }

    fn create(options: Option<WampDict>) -> Self where Self: OptionBuilder + Sized;
    fn get_dict(&self) -> Option<WampDict>;
    
}

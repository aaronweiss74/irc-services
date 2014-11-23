#![cfg(feature = "democracy")]
use std::collections::HashMap;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::io::IoResult;
use std::string::ToString;
use data::channel::Channel;
use irc::data::kinds::IrcStream;
use irc::server::Server;
use irc::server::utils::Wrapper;

pub struct Democracy {
    proposals: HashMap<u8, Proposal>,
    votes: HashMap<String, Vec<Vote>>,
    last_proposal: u8,
}

impl Democracy {
    pub fn new() -> Democracy {
        Democracy {
            proposals: HashMap::new(), 
            votes: HashMap::new(),
            last_proposal: 0 
        }
    }

    pub fn propose(&mut self, proposal: &str, parameter: &str) -> Option<u8> {
        if !self.proposals.is_empty() { self.last_proposal += 1 }
        let proposal = Proposal::from_str(proposal, parameter);
        if let Some(proposal) = proposal {
            self.proposals.insert(self.last_proposal, proposal);
            Some(self.last_proposal)
        } else {
            None
        }
    }

    pub fn has_voted(&self, proposal_id: u8, user: &str) -> bool {
        if let Some(votes) = self.votes.get(&user.into_string()) {
            for vote in votes.iter() {
                match vote {
                    &Vote::Yea(id) if id == proposal_id => return true,
                    &Vote::Nay(id) if id == proposal_id => return true,
                    _ => (),
                }
            }
        }
        false
    }

    pub fn vote(&mut self, proposal_id: u8, vote: &str, user: &str) -> VotingResult {
        let vote = Vote::from_str(vote, proposal_id);
        if vote.is_none() {
            VotingResult::InvalidVote
        } else if self.proposals.contains_key(&proposal_id) {
            match self.votes.entry(user.into_string()) {
                Occupied(mut entry) => entry.get_mut().push(vote.unwrap()),
                Vacant(entry) => { entry.set(vec![vote.unwrap()]); },
            }
            VotingResult::VoteIssued
        } else {
            VotingResult::NoSuchProposal
        }
    }

    pub fn get_result_of_vote(&mut self, proposal_id: u8, voting_pop: uint) -> VoteResult {
        if self.proposals.contains_key(&proposal_id) {
            let full = self.proposals[proposal_id].is_full_vote();
            let mut yea: uint = 0; let mut nay: uint = 0;
            for votes in self.votes.values() {
                match self.find_vote(votes, proposal_id) {
                    Some(Vote::Yea(_)) => yea += 1,
                    Some(Vote::Nay(_)) => nay += 1,
                    None => ()
                }
            }
            if full && (yea * 100) / voting_pop >= 40 || !full && (yea * 100) / voting_pop >= 25 {
                self.delete_all_votes(proposal_id);
                VoteResult::VotePassed(self.proposals.remove(&proposal_id).unwrap())
            } else if full && (nay * 100) / voting_pop >= 60 || !full && (nay * 100) / voting_pop >= 75 {
                self.proposals.remove(&proposal_id);
                self.delete_all_votes(proposal_id);
                VoteResult::VoteFailed
            } else {
                VoteResult::VoteInProgress
            }
        } else {
            VoteResult::NoSuchVote
        }
    }

    pub fn is_full_vote(&self, proposal_id: u8) -> bool {
        self.proposals.get(&proposal_id).map(|p| p.is_full_vote()).unwrap_or(false)
    }

    fn delete_all_votes(&mut self, proposal_id: u8) {
        for (_, votes) in self.votes.iter_mut() {
            votes.retain(|v| v.id() != proposal_id)
        }
    }

    fn find_vote(&self, votes: &Vec<Vote>, proposal_id: u8) -> Option<Vote> {
        for vote in votes.iter() {
            match vote {
                &Vote::Yea(id) if id == proposal_id => return Some(Vote::Yea(id)),
                &Vote::Nay(id) if id == proposal_id => return Some(Vote::Nay(id)),
                _ => (),
            }
        }
        None
    }
}

pub enum VotingResult {
    VoteIssued,
    InvalidVote,
    NoSuchProposal,
}

pub enum VoteResult {
    VotePassed(Proposal),
    VoteFailed,
    VoteInProgress,
    NoSuchVote,
}

enum Vote {
    Yea(u8),
    Nay(u8)
}

impl Vote {
    pub fn from_str(vote: &str, proposal_id: u8) -> Option<Vote> {
        match vote {
            "yea" => Some(Vote::Yea(proposal_id)),
            "nay" => Some(Vote::Nay(proposal_id)),
            _     => None,
        }
    }

    pub fn id(&self) -> u8 {
        match self {
            &Vote::Yea(id) => id,
            &Vote::Nay(id) => id,
        }
    }
}

enum Proposal {
    ChangeOwner(String),
    Oper(String),
    Deop(String),
    // TODO: add ban tracking to channels.
    // Ban(String),
    // Unban(String),
    Kick(String),
    Topic(String),
    Mode(String),
}

impl ToString for Proposal {
    fn to_string(&self) -> String {
        format!("{}", match self {
            &Proposal::ChangeOwner(ref owner) => format!("change the owner to {}", owner[]),
            &Proposal::Oper(ref user) => format!("oper {}", user[]),
            &Proposal::Deop(ref user) => format!("deop {}", user[]),
            &Proposal::Kick(ref user) => format!("kick {}", user[]),
            &Proposal::Topic(ref message) => format!("change the topic to {}", message[]),
            &Proposal::Mode(ref mode) => format!("change the channel mode to {}", mode[]),
        })
    }
}

impl Proposal {
     pub fn from_str(proposal: &str, parameter: &str) -> Option<Proposal> {
        let param = parameter.into_string();
        match proposal {
            "chown" => Some(Proposal::ChangeOwner(param)),
            "oper"  => Some(Proposal::Oper(param)),
            "deop"  => Some(Proposal::Deop(param)),
            "kick"  => Some(Proposal::Kick(param)),
            "topic" => Some(Proposal::Topic(param)),
            "mode"  => Some(Proposal::Mode(param)),
            _       => None
        }
    }

    fn is_full_vote(&self) -> bool {
        match self {
            &Proposal::ChangeOwner(_) => true,
            &Proposal::Oper(_)        => true,
            &Proposal::Deop(_)        => true,
            _                         => false,
        }
    }
}

impl<'a, T> Proposal where T: IrcStream {
    pub fn enact(&self, server: &'a Wrapper<'a, T>, chan: &str) -> IoResult<()> {
        if let Ok(mut channel) = Channel::load(chan) {
            match self {
                &Proposal::ChangeOwner(ref owner) => {
                    channel.owner = owner.clone();
                    try!(channel.save());
                },
                &Proposal::Oper(ref user) => {
                    if user[] == server.config().nickname[] {
                        return server.send_privmsg(chan, "Votes about me cannot be enacted.");
                    }
                    channel.opers.push(user.clone());
                    try!(channel.save());
                    try!(server.send_samode(chan, "+o", user[]));
                },
                &Proposal::Deop(ref user) => {
                    if user[] == server.config().nickname[] {
                        return server.send_privmsg(chan, "Votes about me cannot be enacted.");
                    }
                    channel.opers.retain(|u| u[] != user[]);
                    try!(channel.save());
                    try!(server.send_samode(chan, "-o", user[]));
                },
                &Proposal::Kick(ref user) => {
                    if user[] == server.config().nickname[] {
                        return server.send_privmsg(chan, "Votes about me cannot be enacted.");
                    }
                    try!(server.send_kick(chan, user[], "It was decided so."));
                },
                &Proposal::Topic(ref topic) => {
                    try!(server.send_topic(chan, topic[]));
                },
                &Proposal::Mode(ref mode) => {
                    channel.mode = mode.clone();
                    try!(server.send_samode(chan, mode[], ""));
                },
            }
            server.send_privmsg(chan, format!("Enacted proposal to {}.", self.to_string())[])
        } else {
            server.send_privmsg(chan, 
                                format!("Failed to enact proposal to {}.", self.to_string())[])
        }
    }
}

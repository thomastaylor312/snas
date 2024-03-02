# Simple NATS Auth Service (SNAS)

A globally scalable simple directory service for managing users, built on top of NATS.

## Why does this exist?

Well, even I can admit that this could definitely be [bad idea](https://tenor.com/bAgKW.gif) seeing
as there are plenty of tried and true directory tools out there. However, I was motivated to try
this for two main reasons (beyond having fun writing it):

1. Running things like LDAP and Active Directory requires a whole bunch of time and it is hard to
   run it HA. I just wanted to have a simple directory for my homelab that was pretty hands off
2. NATS is _really_ good at acting as both a simple data store and as something that can be easily
   clustered and HA, as well as being globally available if done right. Although I just plan to run
   a few NATS servers locally for this, it can easily be used in multiple areas or other homelabs
   via tools like [Tailscale](https://tailscale.com/) or [Synadia
   Cloud](https://www.synadia.com/cloud) (née NGS). This can even enable you to spread out replicas
   of the data to anywhere you want.

## Is this ready to use?

It is getting closer! Currently I have the API all built out for NATS, along with e2e tests. I still
need to build the socket API and then PAM modules for linux. I'll try to keep this README up to date
with the current status, or you can also follow along in the files. If for some reason this project
interests you, please reach out and let me know! It will give me motivation to work on it even more.

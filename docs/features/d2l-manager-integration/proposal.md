we've got new tooling: /home/lmb/Desktop/projects/d2l-class-manager 

I want to integrate it into the school dashboard as an optional plugin. Essentially if we see that user has d2l manager installed in their path, then we enable this feature.

What I want the feature include:
1. reconcile changes in deadlines from d2l itself and the deadlines json we maintain: reason being is that the json deadlines is like our cache
2. I want a new ai summary section that takes into account anouncements and what's current happening
3. since some classes have readings required I want to be able to get a summary of what the readings are for next class and what the stories are about if links are provided in the brightspace, this is specifically important for 3020, and I think it's best we have a dedicated tags for class types, so for example 3020 would have a tag of readings and so it would enable that feature

also just a side note ensure we are using flash 3.8 low model for ai summaries

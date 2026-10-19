// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'types.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$CoreEvent {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'CoreEvent()';
}


}

/// @nodoc
class $CoreEventCopyWith<$Res>  {
$CoreEventCopyWith(CoreEvent _, $Res Function(CoreEvent) __);
}


/// Adds pattern-matching-related methods to [CoreEvent].
extension CoreEventPatterns on CoreEvent {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( CoreEvent_MessageReceived value)?  messageReceived,TResult Function( CoreEvent_MessageStatus value)?  messageStatus,TResult Function( CoreEvent_ContactOnline value)?  contactOnline,TResult Function( CoreEvent_ContactOffline value)?  contactOffline,TResult Function( CoreEvent_ContactRequest value)?  contactRequest,TResult Function( CoreEvent_Signal value)?  signal,TResult Function( CoreEvent_NetworkStatus value)?  networkStatus,TResult Function( CoreEvent_KeyChanged value)?  keyChanged,TResult Function( CoreEvent_Server value)?  server,required TResult orElse(),}){
final _that = this;
switch (_that) {
case CoreEvent_MessageReceived() when messageReceived != null:
return messageReceived(_that);case CoreEvent_MessageStatus() when messageStatus != null:
return messageStatus(_that);case CoreEvent_ContactOnline() when contactOnline != null:
return contactOnline(_that);case CoreEvent_ContactOffline() when contactOffline != null:
return contactOffline(_that);case CoreEvent_ContactRequest() when contactRequest != null:
return contactRequest(_that);case CoreEvent_Signal() when signal != null:
return signal(_that);case CoreEvent_NetworkStatus() when networkStatus != null:
return networkStatus(_that);case CoreEvent_KeyChanged() when keyChanged != null:
return keyChanged(_that);case CoreEvent_Server() when server != null:
return server(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( CoreEvent_MessageReceived value)  messageReceived,required TResult Function( CoreEvent_MessageStatus value)  messageStatus,required TResult Function( CoreEvent_ContactOnline value)  contactOnline,required TResult Function( CoreEvent_ContactOffline value)  contactOffline,required TResult Function( CoreEvent_ContactRequest value)  contactRequest,required TResult Function( CoreEvent_Signal value)  signal,required TResult Function( CoreEvent_NetworkStatus value)  networkStatus,required TResult Function( CoreEvent_KeyChanged value)  keyChanged,required TResult Function( CoreEvent_Server value)  server,}){
final _that = this;
switch (_that) {
case CoreEvent_MessageReceived():
return messageReceived(_that);case CoreEvent_MessageStatus():
return messageStatus(_that);case CoreEvent_ContactOnline():
return contactOnline(_that);case CoreEvent_ContactOffline():
return contactOffline(_that);case CoreEvent_ContactRequest():
return contactRequest(_that);case CoreEvent_Signal():
return signal(_that);case CoreEvent_NetworkStatus():
return networkStatus(_that);case CoreEvent_KeyChanged():
return keyChanged(_that);case CoreEvent_Server():
return server(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( CoreEvent_MessageReceived value)?  messageReceived,TResult? Function( CoreEvent_MessageStatus value)?  messageStatus,TResult? Function( CoreEvent_ContactOnline value)?  contactOnline,TResult? Function( CoreEvent_ContactOffline value)?  contactOffline,TResult? Function( CoreEvent_ContactRequest value)?  contactRequest,TResult? Function( CoreEvent_Signal value)?  signal,TResult? Function( CoreEvent_NetworkStatus value)?  networkStatus,TResult? Function( CoreEvent_KeyChanged value)?  keyChanged,TResult? Function( CoreEvent_Server value)?  server,}){
final _that = this;
switch (_that) {
case CoreEvent_MessageReceived() when messageReceived != null:
return messageReceived(_that);case CoreEvent_MessageStatus() when messageStatus != null:
return messageStatus(_that);case CoreEvent_ContactOnline() when contactOnline != null:
return contactOnline(_that);case CoreEvent_ContactOffline() when contactOffline != null:
return contactOffline(_that);case CoreEvent_ContactRequest() when contactRequest != null:
return contactRequest(_that);case CoreEvent_Signal() when signal != null:
return signal(_that);case CoreEvent_NetworkStatus() when networkStatus != null:
return networkStatus(_that);case CoreEvent_KeyChanged() when keyChanged != null:
return keyChanged(_that);case CoreEvent_Server() when server != null:
return server(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( Message message)?  messageReceived,TResult Function( String id,  String peerId,  DeliveryStatus status)?  messageStatus,TResult Function( String peerId)?  contactOnline,TResult Function( String peerId)?  contactOffline,TResult Function( ContactCard card)?  contactRequest,TResult Function( String peerId,  String callId,  SignalKind kind,  String payload)?  signal,TResult Function( NetworkInfo info)?  networkStatus,TResult Function( String peerId,  String oldIdentityKey,  String newIdentityKey,  String oldSafetyNumber,  String newSafetyNumber,  KeyChangeSource source)?  keyChanged,TResult Function( String serverId,  ServerEvent event)?  server,required TResult orElse(),}) {final _that = this;
switch (_that) {
case CoreEvent_MessageReceived() when messageReceived != null:
return messageReceived(_that.message);case CoreEvent_MessageStatus() when messageStatus != null:
return messageStatus(_that.id,_that.peerId,_that.status);case CoreEvent_ContactOnline() when contactOnline != null:
return contactOnline(_that.peerId);case CoreEvent_ContactOffline() when contactOffline != null:
return contactOffline(_that.peerId);case CoreEvent_ContactRequest() when contactRequest != null:
return contactRequest(_that.card);case CoreEvent_Signal() when signal != null:
return signal(_that.peerId,_that.callId,_that.kind,_that.payload);case CoreEvent_NetworkStatus() when networkStatus != null:
return networkStatus(_that.info);case CoreEvent_KeyChanged() when keyChanged != null:
return keyChanged(_that.peerId,_that.oldIdentityKey,_that.newIdentityKey,_that.oldSafetyNumber,_that.newSafetyNumber,_that.source);case CoreEvent_Server() when server != null:
return server(_that.serverId,_that.event);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( Message message)  messageReceived,required TResult Function( String id,  String peerId,  DeliveryStatus status)  messageStatus,required TResult Function( String peerId)  contactOnline,required TResult Function( String peerId)  contactOffline,required TResult Function( ContactCard card)  contactRequest,required TResult Function( String peerId,  String callId,  SignalKind kind,  String payload)  signal,required TResult Function( NetworkInfo info)  networkStatus,required TResult Function( String peerId,  String oldIdentityKey,  String newIdentityKey,  String oldSafetyNumber,  String newSafetyNumber,  KeyChangeSource source)  keyChanged,required TResult Function( String serverId,  ServerEvent event)  server,}) {final _that = this;
switch (_that) {
case CoreEvent_MessageReceived():
return messageReceived(_that.message);case CoreEvent_MessageStatus():
return messageStatus(_that.id,_that.peerId,_that.status);case CoreEvent_ContactOnline():
return contactOnline(_that.peerId);case CoreEvent_ContactOffline():
return contactOffline(_that.peerId);case CoreEvent_ContactRequest():
return contactRequest(_that.card);case CoreEvent_Signal():
return signal(_that.peerId,_that.callId,_that.kind,_that.payload);case CoreEvent_NetworkStatus():
return networkStatus(_that.info);case CoreEvent_KeyChanged():
return keyChanged(_that.peerId,_that.oldIdentityKey,_that.newIdentityKey,_that.oldSafetyNumber,_that.newSafetyNumber,_that.source);case CoreEvent_Server():
return server(_that.serverId,_that.event);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( Message message)?  messageReceived,TResult? Function( String id,  String peerId,  DeliveryStatus status)?  messageStatus,TResult? Function( String peerId)?  contactOnline,TResult? Function( String peerId)?  contactOffline,TResult? Function( ContactCard card)?  contactRequest,TResult? Function( String peerId,  String callId,  SignalKind kind,  String payload)?  signal,TResult? Function( NetworkInfo info)?  networkStatus,TResult? Function( String peerId,  String oldIdentityKey,  String newIdentityKey,  String oldSafetyNumber,  String newSafetyNumber,  KeyChangeSource source)?  keyChanged,TResult? Function( String serverId,  ServerEvent event)?  server,}) {final _that = this;
switch (_that) {
case CoreEvent_MessageReceived() when messageReceived != null:
return messageReceived(_that.message);case CoreEvent_MessageStatus() when messageStatus != null:
return messageStatus(_that.id,_that.peerId,_that.status);case CoreEvent_ContactOnline() when contactOnline != null:
return contactOnline(_that.peerId);case CoreEvent_ContactOffline() when contactOffline != null:
return contactOffline(_that.peerId);case CoreEvent_ContactRequest() when contactRequest != null:
return contactRequest(_that.card);case CoreEvent_Signal() when signal != null:
return signal(_that.peerId,_that.callId,_that.kind,_that.payload);case CoreEvent_NetworkStatus() when networkStatus != null:
return networkStatus(_that.info);case CoreEvent_KeyChanged() when keyChanged != null:
return keyChanged(_that.peerId,_that.oldIdentityKey,_that.newIdentityKey,_that.oldSafetyNumber,_that.newSafetyNumber,_that.source);case CoreEvent_Server() when server != null:
return server(_that.serverId,_that.event);case _:
  return null;

}
}

}

/// @nodoc


class CoreEvent_MessageReceived extends CoreEvent {
  const CoreEvent_MessageReceived({required this.message}): super._();
  

 final  Message message;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_MessageReceivedCopyWith<CoreEvent_MessageReceived> get copyWith => _$CoreEvent_MessageReceivedCopyWithImpl<CoreEvent_MessageReceived>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_MessageReceived&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,message);

@override
String toString() {
  return 'CoreEvent.messageReceived(message: $message)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_MessageReceivedCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_MessageReceivedCopyWith(CoreEvent_MessageReceived value, $Res Function(CoreEvent_MessageReceived) _then) = _$CoreEvent_MessageReceivedCopyWithImpl;
@useResult
$Res call({
 Message message
});




}
/// @nodoc
class _$CoreEvent_MessageReceivedCopyWithImpl<$Res>
    implements $CoreEvent_MessageReceivedCopyWith<$Res> {
  _$CoreEvent_MessageReceivedCopyWithImpl(this._self, this._then);

  final CoreEvent_MessageReceived _self;
  final $Res Function(CoreEvent_MessageReceived) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(CoreEvent_MessageReceived(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as Message,
  ));
}


}

/// @nodoc


class CoreEvent_MessageStatus extends CoreEvent {
  const CoreEvent_MessageStatus({required this.id, required this.peerId, required this.status}): super._();
  

 final  String id;
 final  String peerId;
 final  DeliveryStatus status;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_MessageStatusCopyWith<CoreEvent_MessageStatus> get copyWith => _$CoreEvent_MessageStatusCopyWithImpl<CoreEvent_MessageStatus>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_MessageStatus&&(identical(other.id, id) || other.id == id)&&(identical(other.peerId, peerId) || other.peerId == peerId)&&(identical(other.status, status) || other.status == status));
}


@override
int get hashCode => Object.hash(runtimeType,id,peerId,status);

@override
String toString() {
  return 'CoreEvent.messageStatus(id: $id, peerId: $peerId, status: $status)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_MessageStatusCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_MessageStatusCopyWith(CoreEvent_MessageStatus value, $Res Function(CoreEvent_MessageStatus) _then) = _$CoreEvent_MessageStatusCopyWithImpl;
@useResult
$Res call({
 String id, String peerId, DeliveryStatus status
});




}
/// @nodoc
class _$CoreEvent_MessageStatusCopyWithImpl<$Res>
    implements $CoreEvent_MessageStatusCopyWith<$Res> {
  _$CoreEvent_MessageStatusCopyWithImpl(this._self, this._then);

  final CoreEvent_MessageStatus _self;
  final $Res Function(CoreEvent_MessageStatus) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? id = null,Object? peerId = null,Object? status = null,}) {
  return _then(CoreEvent_MessageStatus(
id: null == id ? _self.id : id // ignore: cast_nullable_to_non_nullable
as String,peerId: null == peerId ? _self.peerId : peerId // ignore: cast_nullable_to_non_nullable
as String,status: null == status ? _self.status : status // ignore: cast_nullable_to_non_nullable
as DeliveryStatus,
  ));
}


}

/// @nodoc


class CoreEvent_ContactOnline extends CoreEvent {
  const CoreEvent_ContactOnline({required this.peerId}): super._();
  

 final  String peerId;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_ContactOnlineCopyWith<CoreEvent_ContactOnline> get copyWith => _$CoreEvent_ContactOnlineCopyWithImpl<CoreEvent_ContactOnline>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_ContactOnline&&(identical(other.peerId, peerId) || other.peerId == peerId));
}


@override
int get hashCode => Object.hash(runtimeType,peerId);

@override
String toString() {
  return 'CoreEvent.contactOnline(peerId: $peerId)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_ContactOnlineCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_ContactOnlineCopyWith(CoreEvent_ContactOnline value, $Res Function(CoreEvent_ContactOnline) _then) = _$CoreEvent_ContactOnlineCopyWithImpl;
@useResult
$Res call({
 String peerId
});




}
/// @nodoc
class _$CoreEvent_ContactOnlineCopyWithImpl<$Res>
    implements $CoreEvent_ContactOnlineCopyWith<$Res> {
  _$CoreEvent_ContactOnlineCopyWithImpl(this._self, this._then);

  final CoreEvent_ContactOnline _self;
  final $Res Function(CoreEvent_ContactOnline) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? peerId = null,}) {
  return _then(CoreEvent_ContactOnline(
peerId: null == peerId ? _self.peerId : peerId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class CoreEvent_ContactOffline extends CoreEvent {
  const CoreEvent_ContactOffline({required this.peerId}): super._();
  

 final  String peerId;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_ContactOfflineCopyWith<CoreEvent_ContactOffline> get copyWith => _$CoreEvent_ContactOfflineCopyWithImpl<CoreEvent_ContactOffline>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_ContactOffline&&(identical(other.peerId, peerId) || other.peerId == peerId));
}


@override
int get hashCode => Object.hash(runtimeType,peerId);

@override
String toString() {
  return 'CoreEvent.contactOffline(peerId: $peerId)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_ContactOfflineCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_ContactOfflineCopyWith(CoreEvent_ContactOffline value, $Res Function(CoreEvent_ContactOffline) _then) = _$CoreEvent_ContactOfflineCopyWithImpl;
@useResult
$Res call({
 String peerId
});




}
/// @nodoc
class _$CoreEvent_ContactOfflineCopyWithImpl<$Res>
    implements $CoreEvent_ContactOfflineCopyWith<$Res> {
  _$CoreEvent_ContactOfflineCopyWithImpl(this._self, this._then);

  final CoreEvent_ContactOffline _self;
  final $Res Function(CoreEvent_ContactOffline) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? peerId = null,}) {
  return _then(CoreEvent_ContactOffline(
peerId: null == peerId ? _self.peerId : peerId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class CoreEvent_ContactRequest extends CoreEvent {
  const CoreEvent_ContactRequest({required this.card}): super._();
  

 final  ContactCard card;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_ContactRequestCopyWith<CoreEvent_ContactRequest> get copyWith => _$CoreEvent_ContactRequestCopyWithImpl<CoreEvent_ContactRequest>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_ContactRequest&&(identical(other.card, card) || other.card == card));
}


@override
int get hashCode => Object.hash(runtimeType,card);

@override
String toString() {
  return 'CoreEvent.contactRequest(card: $card)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_ContactRequestCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_ContactRequestCopyWith(CoreEvent_ContactRequest value, $Res Function(CoreEvent_ContactRequest) _then) = _$CoreEvent_ContactRequestCopyWithImpl;
@useResult
$Res call({
 ContactCard card
});




}
/// @nodoc
class _$CoreEvent_ContactRequestCopyWithImpl<$Res>
    implements $CoreEvent_ContactRequestCopyWith<$Res> {
  _$CoreEvent_ContactRequestCopyWithImpl(this._self, this._then);

  final CoreEvent_ContactRequest _self;
  final $Res Function(CoreEvent_ContactRequest) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? card = null,}) {
  return _then(CoreEvent_ContactRequest(
card: null == card ? _self.card : card // ignore: cast_nullable_to_non_nullable
as ContactCard,
  ));
}


}

/// @nodoc


class CoreEvent_Signal extends CoreEvent {
  const CoreEvent_Signal({required this.peerId, required this.callId, required this.kind, required this.payload}): super._();
  

 final  String peerId;
 final  String callId;
 final  SignalKind kind;
 final  String payload;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_SignalCopyWith<CoreEvent_Signal> get copyWith => _$CoreEvent_SignalCopyWithImpl<CoreEvent_Signal>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_Signal&&(identical(other.peerId, peerId) || other.peerId == peerId)&&(identical(other.callId, callId) || other.callId == callId)&&(identical(other.kind, kind) || other.kind == kind)&&(identical(other.payload, payload) || other.payload == payload));
}


@override
int get hashCode => Object.hash(runtimeType,peerId,callId,kind,payload);

@override
String toString() {
  return 'CoreEvent.signal(peerId: $peerId, callId: $callId, kind: $kind, payload: $payload)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_SignalCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_SignalCopyWith(CoreEvent_Signal value, $Res Function(CoreEvent_Signal) _then) = _$CoreEvent_SignalCopyWithImpl;
@useResult
$Res call({
 String peerId, String callId, SignalKind kind, String payload
});




}
/// @nodoc
class _$CoreEvent_SignalCopyWithImpl<$Res>
    implements $CoreEvent_SignalCopyWith<$Res> {
  _$CoreEvent_SignalCopyWithImpl(this._self, this._then);

  final CoreEvent_Signal _self;
  final $Res Function(CoreEvent_Signal) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? peerId = null,Object? callId = null,Object? kind = null,Object? payload = null,}) {
  return _then(CoreEvent_Signal(
peerId: null == peerId ? _self.peerId : peerId // ignore: cast_nullable_to_non_nullable
as String,callId: null == callId ? _self.callId : callId // ignore: cast_nullable_to_non_nullable
as String,kind: null == kind ? _self.kind : kind // ignore: cast_nullable_to_non_nullable
as SignalKind,payload: null == payload ? _self.payload : payload // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class CoreEvent_NetworkStatus extends CoreEvent {
  const CoreEvent_NetworkStatus({required this.info}): super._();
  

 final  NetworkInfo info;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_NetworkStatusCopyWith<CoreEvent_NetworkStatus> get copyWith => _$CoreEvent_NetworkStatusCopyWithImpl<CoreEvent_NetworkStatus>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_NetworkStatus&&(identical(other.info, info) || other.info == info));
}


@override
int get hashCode => Object.hash(runtimeType,info);

@override
String toString() {
  return 'CoreEvent.networkStatus(info: $info)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_NetworkStatusCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_NetworkStatusCopyWith(CoreEvent_NetworkStatus value, $Res Function(CoreEvent_NetworkStatus) _then) = _$CoreEvent_NetworkStatusCopyWithImpl;
@useResult
$Res call({
 NetworkInfo info
});




}
/// @nodoc
class _$CoreEvent_NetworkStatusCopyWithImpl<$Res>
    implements $CoreEvent_NetworkStatusCopyWith<$Res> {
  _$CoreEvent_NetworkStatusCopyWithImpl(this._self, this._then);

  final CoreEvent_NetworkStatus _self;
  final $Res Function(CoreEvent_NetworkStatus) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? info = null,}) {
  return _then(CoreEvent_NetworkStatus(
info: null == info ? _self.info : info // ignore: cast_nullable_to_non_nullable
as NetworkInfo,
  ));
}


}

/// @nodoc


class CoreEvent_KeyChanged extends CoreEvent {
  const CoreEvent_KeyChanged({required this.peerId, required this.oldIdentityKey, required this.newIdentityKey, required this.oldSafetyNumber, required this.newSafetyNumber, required this.source}): super._();
  

 final  String peerId;
 final  String oldIdentityKey;
 final  String newIdentityKey;
 final  String oldSafetyNumber;
 final  String newSafetyNumber;
 final  KeyChangeSource source;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_KeyChangedCopyWith<CoreEvent_KeyChanged> get copyWith => _$CoreEvent_KeyChangedCopyWithImpl<CoreEvent_KeyChanged>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_KeyChanged&&(identical(other.peerId, peerId) || other.peerId == peerId)&&(identical(other.oldIdentityKey, oldIdentityKey) || other.oldIdentityKey == oldIdentityKey)&&(identical(other.newIdentityKey, newIdentityKey) || other.newIdentityKey == newIdentityKey)&&(identical(other.oldSafetyNumber, oldSafetyNumber) || other.oldSafetyNumber == oldSafetyNumber)&&(identical(other.newSafetyNumber, newSafetyNumber) || other.newSafetyNumber == newSafetyNumber)&&(identical(other.source, source) || other.source == source));
}


@override
int get hashCode => Object.hash(runtimeType,peerId,oldIdentityKey,newIdentityKey,oldSafetyNumber,newSafetyNumber,source);

@override
String toString() {
  return 'CoreEvent.keyChanged(peerId: $peerId, oldIdentityKey: $oldIdentityKey, newIdentityKey: $newIdentityKey, oldSafetyNumber: $oldSafetyNumber, newSafetyNumber: $newSafetyNumber, source: $source)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_KeyChangedCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_KeyChangedCopyWith(CoreEvent_KeyChanged value, $Res Function(CoreEvent_KeyChanged) _then) = _$CoreEvent_KeyChangedCopyWithImpl;
@useResult
$Res call({
 String peerId, String oldIdentityKey, String newIdentityKey, String oldSafetyNumber, String newSafetyNumber, KeyChangeSource source
});




}
/// @nodoc
class _$CoreEvent_KeyChangedCopyWithImpl<$Res>
    implements $CoreEvent_KeyChangedCopyWith<$Res> {
  _$CoreEvent_KeyChangedCopyWithImpl(this._self, this._then);

  final CoreEvent_KeyChanged _self;
  final $Res Function(CoreEvent_KeyChanged) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? peerId = null,Object? oldIdentityKey = null,Object? newIdentityKey = null,Object? oldSafetyNumber = null,Object? newSafetyNumber = null,Object? source = null,}) {
  return _then(CoreEvent_KeyChanged(
peerId: null == peerId ? _self.peerId : peerId // ignore: cast_nullable_to_non_nullable
as String,oldIdentityKey: null == oldIdentityKey ? _self.oldIdentityKey : oldIdentityKey // ignore: cast_nullable_to_non_nullable
as String,newIdentityKey: null == newIdentityKey ? _self.newIdentityKey : newIdentityKey // ignore: cast_nullable_to_non_nullable
as String,oldSafetyNumber: null == oldSafetyNumber ? _self.oldSafetyNumber : oldSafetyNumber // ignore: cast_nullable_to_non_nullable
as String,newSafetyNumber: null == newSafetyNumber ? _self.newSafetyNumber : newSafetyNumber // ignore: cast_nullable_to_non_nullable
as String,source: null == source ? _self.source : source // ignore: cast_nullable_to_non_nullable
as KeyChangeSource,
  ));
}


}

/// @nodoc


class CoreEvent_Server extends CoreEvent {
  const CoreEvent_Server({required this.serverId, required this.event}): super._();
  

 final  String serverId;
 final  ServerEvent event;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_ServerCopyWith<CoreEvent_Server> get copyWith => _$CoreEvent_ServerCopyWithImpl<CoreEvent_Server>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_Server&&(identical(other.serverId, serverId) || other.serverId == serverId)&&(identical(other.event, event) || other.event == event));
}


@override
int get hashCode => Object.hash(runtimeType,serverId,event);

@override
String toString() {
  return 'CoreEvent.server(serverId: $serverId, event: $event)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_ServerCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_ServerCopyWith(CoreEvent_Server value, $Res Function(CoreEvent_Server) _then) = _$CoreEvent_ServerCopyWithImpl;
@useResult
$Res call({
 String serverId, ServerEvent event
});


$ServerEventCopyWith<$Res> get event;

}
/// @nodoc
class _$CoreEvent_ServerCopyWithImpl<$Res>
    implements $CoreEvent_ServerCopyWith<$Res> {
  _$CoreEvent_ServerCopyWithImpl(this._self, this._then);

  final CoreEvent_Server _self;
  final $Res Function(CoreEvent_Server) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? serverId = null,Object? event = null,}) {
  return _then(CoreEvent_Server(
serverId: null == serverId ? _self.serverId : serverId // ignore: cast_nullable_to_non_nullable
as String,event: null == event ? _self.event : event // ignore: cast_nullable_to_non_nullable
as ServerEvent,
  ));
}

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@override
@pragma('vm:prefer-inline')
$ServerEventCopyWith<$Res> get event {
  
  return $ServerEventCopyWith<$Res>(_self.event, (value) {
    return _then(_self.copyWith(event: value));
  });
}
}

/// @nodoc
mixin _$ServerEvent {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ServerEvent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ServerEvent()';
}


}

/// @nodoc
class $ServerEventCopyWith<$Res>  {
$ServerEventCopyWith(ServerEvent _, $Res Function(ServerEvent) __);
}


/// Adds pattern-matching-related methods to [ServerEvent].
extension ServerEventPatterns on ServerEvent {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( ServerEvent_Joined value)?  joined,TResult Function( ServerEvent_Updated value)?  updated,TResult Function( ServerEvent_MemberJoined value)?  memberJoined,TResult Function( ServerEvent_MemberLeft value)?  memberLeft,TResult Function( ServerEvent_JoinRequest value)?  joinRequest,TResult Function( ServerEvent_ManifestRollback value)?  manifestRollback,TResult Function( ServerEvent_Forked value)?  forked,TResult Function( ServerEvent_Removed value)?  removed,required TResult orElse(),}){
final _that = this;
switch (_that) {
case ServerEvent_Joined() when joined != null:
return joined(_that);case ServerEvent_Updated() when updated != null:
return updated(_that);case ServerEvent_MemberJoined() when memberJoined != null:
return memberJoined(_that);case ServerEvent_MemberLeft() when memberLeft != null:
return memberLeft(_that);case ServerEvent_JoinRequest() when joinRequest != null:
return joinRequest(_that);case ServerEvent_ManifestRollback() when manifestRollback != null:
return manifestRollback(_that);case ServerEvent_Forked() when forked != null:
return forked(_that);case ServerEvent_Removed() when removed != null:
return removed(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( ServerEvent_Joined value)  joined,required TResult Function( ServerEvent_Updated value)  updated,required TResult Function( ServerEvent_MemberJoined value)  memberJoined,required TResult Function( ServerEvent_MemberLeft value)  memberLeft,required TResult Function( ServerEvent_JoinRequest value)  joinRequest,required TResult Function( ServerEvent_ManifestRollback value)  manifestRollback,required TResult Function( ServerEvent_Forked value)  forked,required TResult Function( ServerEvent_Removed value)  removed,}){
final _that = this;
switch (_that) {
case ServerEvent_Joined():
return joined(_that);case ServerEvent_Updated():
return updated(_that);case ServerEvent_MemberJoined():
return memberJoined(_that);case ServerEvent_MemberLeft():
return memberLeft(_that);case ServerEvent_JoinRequest():
return joinRequest(_that);case ServerEvent_ManifestRollback():
return manifestRollback(_that);case ServerEvent_Forked():
return forked(_that);case ServerEvent_Removed():
return removed(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( ServerEvent_Joined value)?  joined,TResult? Function( ServerEvent_Updated value)?  updated,TResult? Function( ServerEvent_MemberJoined value)?  memberJoined,TResult? Function( ServerEvent_MemberLeft value)?  memberLeft,TResult? Function( ServerEvent_JoinRequest value)?  joinRequest,TResult? Function( ServerEvent_ManifestRollback value)?  manifestRollback,TResult? Function( ServerEvent_Forked value)?  forked,TResult? Function( ServerEvent_Removed value)?  removed,}){
final _that = this;
switch (_that) {
case ServerEvent_Joined() when joined != null:
return joined(_that);case ServerEvent_Updated() when updated != null:
return updated(_that);case ServerEvent_MemberJoined() when memberJoined != null:
return memberJoined(_that);case ServerEvent_MemberLeft() when memberLeft != null:
return memberLeft(_that);case ServerEvent_JoinRequest() when joinRequest != null:
return joinRequest(_that);case ServerEvent_ManifestRollback() when manifestRollback != null:
return manifestRollback(_that);case ServerEvent_Forked() when forked != null:
return forked(_that);case ServerEvent_Removed() when removed != null:
return removed(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  joined,TResult Function( BigInt seq)?  updated,TResult Function( String peerId)?  memberJoined,TResult Function( String peerId,  MemberLeftReason reason)?  memberLeft,TResult Function( JoinRequest request)?  joinRequest,TResult Function( BigInt have,  BigInt got)?  manifestRollback,TResult Function( BigInt seq)?  forked,TResult Function( MemberLeftReason reason)?  removed,required TResult orElse(),}) {final _that = this;
switch (_that) {
case ServerEvent_Joined() when joined != null:
return joined();case ServerEvent_Updated() when updated != null:
return updated(_that.seq);case ServerEvent_MemberJoined() when memberJoined != null:
return memberJoined(_that.peerId);case ServerEvent_MemberLeft() when memberLeft != null:
return memberLeft(_that.peerId,_that.reason);case ServerEvent_JoinRequest() when joinRequest != null:
return joinRequest(_that.request);case ServerEvent_ManifestRollback() when manifestRollback != null:
return manifestRollback(_that.have,_that.got);case ServerEvent_Forked() when forked != null:
return forked(_that.seq);case ServerEvent_Removed() when removed != null:
return removed(_that.reason);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  joined,required TResult Function( BigInt seq)  updated,required TResult Function( String peerId)  memberJoined,required TResult Function( String peerId,  MemberLeftReason reason)  memberLeft,required TResult Function( JoinRequest request)  joinRequest,required TResult Function( BigInt have,  BigInt got)  manifestRollback,required TResult Function( BigInt seq)  forked,required TResult Function( MemberLeftReason reason)  removed,}) {final _that = this;
switch (_that) {
case ServerEvent_Joined():
return joined();case ServerEvent_Updated():
return updated(_that.seq);case ServerEvent_MemberJoined():
return memberJoined(_that.peerId);case ServerEvent_MemberLeft():
return memberLeft(_that.peerId,_that.reason);case ServerEvent_JoinRequest():
return joinRequest(_that.request);case ServerEvent_ManifestRollback():
return manifestRollback(_that.have,_that.got);case ServerEvent_Forked():
return forked(_that.seq);case ServerEvent_Removed():
return removed(_that.reason);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  joined,TResult? Function( BigInt seq)?  updated,TResult? Function( String peerId)?  memberJoined,TResult? Function( String peerId,  MemberLeftReason reason)?  memberLeft,TResult? Function( JoinRequest request)?  joinRequest,TResult? Function( BigInt have,  BigInt got)?  manifestRollback,TResult? Function( BigInt seq)?  forked,TResult? Function( MemberLeftReason reason)?  removed,}) {final _that = this;
switch (_that) {
case ServerEvent_Joined() when joined != null:
return joined();case ServerEvent_Updated() when updated != null:
return updated(_that.seq);case ServerEvent_MemberJoined() when memberJoined != null:
return memberJoined(_that.peerId);case ServerEvent_MemberLeft() when memberLeft != null:
return memberLeft(_that.peerId,_that.reason);case ServerEvent_JoinRequest() when joinRequest != null:
return joinRequest(_that.request);case ServerEvent_ManifestRollback() when manifestRollback != null:
return manifestRollback(_that.have,_that.got);case ServerEvent_Forked() when forked != null:
return forked(_that.seq);case ServerEvent_Removed() when removed != null:
return removed(_that.reason);case _:
  return null;

}
}

}

/// @nodoc


class ServerEvent_Joined extends ServerEvent {
  const ServerEvent_Joined(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ServerEvent_Joined);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'ServerEvent.joined()';
}


}




/// @nodoc


class ServerEvent_Updated extends ServerEvent {
  const ServerEvent_Updated({required this.seq}): super._();
  

 final  BigInt seq;

/// Create a copy of ServerEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ServerEvent_UpdatedCopyWith<ServerEvent_Updated> get copyWith => _$ServerEvent_UpdatedCopyWithImpl<ServerEvent_Updated>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ServerEvent_Updated&&(identical(other.seq, seq) || other.seq == seq));
}


@override
int get hashCode => Object.hash(runtimeType,seq);

@override
String toString() {
  return 'ServerEvent.updated(seq: $seq)';
}


}

/// @nodoc
abstract mixin class $ServerEvent_UpdatedCopyWith<$Res> implements $ServerEventCopyWith<$Res> {
  factory $ServerEvent_UpdatedCopyWith(ServerEvent_Updated value, $Res Function(ServerEvent_Updated) _then) = _$ServerEvent_UpdatedCopyWithImpl;
@useResult
$Res call({
 BigInt seq
});




}
/// @nodoc
class _$ServerEvent_UpdatedCopyWithImpl<$Res>
    implements $ServerEvent_UpdatedCopyWith<$Res> {
  _$ServerEvent_UpdatedCopyWithImpl(this._self, this._then);

  final ServerEvent_Updated _self;
  final $Res Function(ServerEvent_Updated) _then;

/// Create a copy of ServerEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? seq = null,}) {
  return _then(ServerEvent_Updated(
seq: null == seq ? _self.seq : seq // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class ServerEvent_MemberJoined extends ServerEvent {
  const ServerEvent_MemberJoined({required this.peerId}): super._();
  

 final  String peerId;

/// Create a copy of ServerEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ServerEvent_MemberJoinedCopyWith<ServerEvent_MemberJoined> get copyWith => _$ServerEvent_MemberJoinedCopyWithImpl<ServerEvent_MemberJoined>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ServerEvent_MemberJoined&&(identical(other.peerId, peerId) || other.peerId == peerId));
}


@override
int get hashCode => Object.hash(runtimeType,peerId);

@override
String toString() {
  return 'ServerEvent.memberJoined(peerId: $peerId)';
}


}

/// @nodoc
abstract mixin class $ServerEvent_MemberJoinedCopyWith<$Res> implements $ServerEventCopyWith<$Res> {
  factory $ServerEvent_MemberJoinedCopyWith(ServerEvent_MemberJoined value, $Res Function(ServerEvent_MemberJoined) _then) = _$ServerEvent_MemberJoinedCopyWithImpl;
@useResult
$Res call({
 String peerId
});




}
/// @nodoc
class _$ServerEvent_MemberJoinedCopyWithImpl<$Res>
    implements $ServerEvent_MemberJoinedCopyWith<$Res> {
  _$ServerEvent_MemberJoinedCopyWithImpl(this._self, this._then);

  final ServerEvent_MemberJoined _self;
  final $Res Function(ServerEvent_MemberJoined) _then;

/// Create a copy of ServerEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? peerId = null,}) {
  return _then(ServerEvent_MemberJoined(
peerId: null == peerId ? _self.peerId : peerId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ServerEvent_MemberLeft extends ServerEvent {
  const ServerEvent_MemberLeft({required this.peerId, required this.reason}): super._();
  

 final  String peerId;
 final  MemberLeftReason reason;

/// Create a copy of ServerEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ServerEvent_MemberLeftCopyWith<ServerEvent_MemberLeft> get copyWith => _$ServerEvent_MemberLeftCopyWithImpl<ServerEvent_MemberLeft>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ServerEvent_MemberLeft&&(identical(other.peerId, peerId) || other.peerId == peerId)&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode => Object.hash(runtimeType,peerId,reason);

@override
String toString() {
  return 'ServerEvent.memberLeft(peerId: $peerId, reason: $reason)';
}


}

/// @nodoc
abstract mixin class $ServerEvent_MemberLeftCopyWith<$Res> implements $ServerEventCopyWith<$Res> {
  factory $ServerEvent_MemberLeftCopyWith(ServerEvent_MemberLeft value, $Res Function(ServerEvent_MemberLeft) _then) = _$ServerEvent_MemberLeftCopyWithImpl;
@useResult
$Res call({
 String peerId, MemberLeftReason reason
});




}
/// @nodoc
class _$ServerEvent_MemberLeftCopyWithImpl<$Res>
    implements $ServerEvent_MemberLeftCopyWith<$Res> {
  _$ServerEvent_MemberLeftCopyWithImpl(this._self, this._then);

  final ServerEvent_MemberLeft _self;
  final $Res Function(ServerEvent_MemberLeft) _then;

/// Create a copy of ServerEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? peerId = null,Object? reason = null,}) {
  return _then(ServerEvent_MemberLeft(
peerId: null == peerId ? _self.peerId : peerId // ignore: cast_nullable_to_non_nullable
as String,reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as MemberLeftReason,
  ));
}


}

/// @nodoc


class ServerEvent_JoinRequest extends ServerEvent {
  const ServerEvent_JoinRequest({required this.request}): super._();
  

 final  JoinRequest request;

/// Create a copy of ServerEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ServerEvent_JoinRequestCopyWith<ServerEvent_JoinRequest> get copyWith => _$ServerEvent_JoinRequestCopyWithImpl<ServerEvent_JoinRequest>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ServerEvent_JoinRequest&&(identical(other.request, request) || other.request == request));
}


@override
int get hashCode => Object.hash(runtimeType,request);

@override
String toString() {
  return 'ServerEvent.joinRequest(request: $request)';
}


}

/// @nodoc
abstract mixin class $ServerEvent_JoinRequestCopyWith<$Res> implements $ServerEventCopyWith<$Res> {
  factory $ServerEvent_JoinRequestCopyWith(ServerEvent_JoinRequest value, $Res Function(ServerEvent_JoinRequest) _then) = _$ServerEvent_JoinRequestCopyWithImpl;
@useResult
$Res call({
 JoinRequest request
});




}
/// @nodoc
class _$ServerEvent_JoinRequestCopyWithImpl<$Res>
    implements $ServerEvent_JoinRequestCopyWith<$Res> {
  _$ServerEvent_JoinRequestCopyWithImpl(this._self, this._then);

  final ServerEvent_JoinRequest _self;
  final $Res Function(ServerEvent_JoinRequest) _then;

/// Create a copy of ServerEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? request = null,}) {
  return _then(ServerEvent_JoinRequest(
request: null == request ? _self.request : request // ignore: cast_nullable_to_non_nullable
as JoinRequest,
  ));
}


}

/// @nodoc


class ServerEvent_ManifestRollback extends ServerEvent {
  const ServerEvent_ManifestRollback({required this.have, required this.got}): super._();
  

 final  BigInt have;
 final  BigInt got;

/// Create a copy of ServerEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ServerEvent_ManifestRollbackCopyWith<ServerEvent_ManifestRollback> get copyWith => _$ServerEvent_ManifestRollbackCopyWithImpl<ServerEvent_ManifestRollback>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ServerEvent_ManifestRollback&&(identical(other.have, have) || other.have == have)&&(identical(other.got, got) || other.got == got));
}


@override
int get hashCode => Object.hash(runtimeType,have,got);

@override
String toString() {
  return 'ServerEvent.manifestRollback(have: $have, got: $got)';
}


}

/// @nodoc
abstract mixin class $ServerEvent_ManifestRollbackCopyWith<$Res> implements $ServerEventCopyWith<$Res> {
  factory $ServerEvent_ManifestRollbackCopyWith(ServerEvent_ManifestRollback value, $Res Function(ServerEvent_ManifestRollback) _then) = _$ServerEvent_ManifestRollbackCopyWithImpl;
@useResult
$Res call({
 BigInt have, BigInt got
});




}
/// @nodoc
class _$ServerEvent_ManifestRollbackCopyWithImpl<$Res>
    implements $ServerEvent_ManifestRollbackCopyWith<$Res> {
  _$ServerEvent_ManifestRollbackCopyWithImpl(this._self, this._then);

  final ServerEvent_ManifestRollback _self;
  final $Res Function(ServerEvent_ManifestRollback) _then;

/// Create a copy of ServerEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? have = null,Object? got = null,}) {
  return _then(ServerEvent_ManifestRollback(
have: null == have ? _self.have : have // ignore: cast_nullable_to_non_nullable
as BigInt,got: null == got ? _self.got : got // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class ServerEvent_Forked extends ServerEvent {
  const ServerEvent_Forked({required this.seq}): super._();
  

 final  BigInt seq;

/// Create a copy of ServerEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ServerEvent_ForkedCopyWith<ServerEvent_Forked> get copyWith => _$ServerEvent_ForkedCopyWithImpl<ServerEvent_Forked>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ServerEvent_Forked&&(identical(other.seq, seq) || other.seq == seq));
}


@override
int get hashCode => Object.hash(runtimeType,seq);

@override
String toString() {
  return 'ServerEvent.forked(seq: $seq)';
}


}

/// @nodoc
abstract mixin class $ServerEvent_ForkedCopyWith<$Res> implements $ServerEventCopyWith<$Res> {
  factory $ServerEvent_ForkedCopyWith(ServerEvent_Forked value, $Res Function(ServerEvent_Forked) _then) = _$ServerEvent_ForkedCopyWithImpl;
@useResult
$Res call({
 BigInt seq
});




}
/// @nodoc
class _$ServerEvent_ForkedCopyWithImpl<$Res>
    implements $ServerEvent_ForkedCopyWith<$Res> {
  _$ServerEvent_ForkedCopyWithImpl(this._self, this._then);

  final ServerEvent_Forked _self;
  final $Res Function(ServerEvent_Forked) _then;

/// Create a copy of ServerEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? seq = null,}) {
  return _then(ServerEvent_Forked(
seq: null == seq ? _self.seq : seq // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class ServerEvent_Removed extends ServerEvent {
  const ServerEvent_Removed({required this.reason}): super._();
  

 final  MemberLeftReason reason;

/// Create a copy of ServerEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ServerEvent_RemovedCopyWith<ServerEvent_Removed> get copyWith => _$ServerEvent_RemovedCopyWithImpl<ServerEvent_Removed>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is ServerEvent_Removed&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode => Object.hash(runtimeType,reason);

@override
String toString() {
  return 'ServerEvent.removed(reason: $reason)';
}


}

/// @nodoc
abstract mixin class $ServerEvent_RemovedCopyWith<$Res> implements $ServerEventCopyWith<$Res> {
  factory $ServerEvent_RemovedCopyWith(ServerEvent_Removed value, $Res Function(ServerEvent_Removed) _then) = _$ServerEvent_RemovedCopyWithImpl;
@useResult
$Res call({
 MemberLeftReason reason
});




}
/// @nodoc
class _$ServerEvent_RemovedCopyWithImpl<$Res>
    implements $ServerEvent_RemovedCopyWith<$Res> {
  _$ServerEvent_RemovedCopyWithImpl(this._self, this._then);

  final ServerEvent_Removed _self;
  final $Res Function(ServerEvent_Removed) _then;

/// Create a copy of ServerEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? reason = null,}) {
  return _then(ServerEvent_Removed(
reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as MemberLeftReason,
  ));
}


}

// dart format on
